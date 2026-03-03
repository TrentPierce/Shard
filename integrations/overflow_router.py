"""
Shard Overflow Router - OpenAI-compatible proxy with GPU-pressure-aware routing.

Usage:
    uvicorn integrations.overflow_router:app --port 8080
"""

from __future__ import annotations

import asyncio
import json
import os
import time
from collections import Counter, deque
from dataclasses import dataclass
from typing import Any, AsyncIterator

import httpx
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, Response, StreamingResponse

try:
    import structlog

    log = structlog.get_logger()
except Exception:  # pragma: no cover - fallback for minimal environments
    import logging

    _base_log = logging.getLogger("overflow_router")
    if not _base_log.handlers:
        logging.basicConfig(level=logging.INFO)

    class _FallbackLogger:
        def info(self, event: str, **kwargs: Any) -> None:
            _base_log.info("%s %s", event, kwargs)

        def warning(self, event: str, **kwargs: Any) -> None:
            _base_log.warning("%s %s", event, kwargs)

    log = _FallbackLogger()


def _now() -> float:
    return time.time()


def _parse_bool_env(name: str, default: bool = False) -> bool:
    raw = os.getenv(name)
    if raw is None:
        return default
    return raw.strip().lower() in {"1", "true", "yes", "on"}


def _parse_float_env(name: str, default: float) -> float:
    raw = os.getenv(name)
    if raw is None:
        return default
    try:
        return float(raw)
    except ValueError:
        return default


@dataclass
class RouterConfig:
    primary_url: str
    shard_url: str
    gpu_threshold: float
    cb_error_threshold: float
    cb_open_duration_seconds: float
    cb_window_seconds: float = 60.0
    cb_half_open_probe_every: int = 10
    request_timeout_seconds: float = 120.0

    @classmethod
    def from_env(cls) -> "RouterConfig":
        return cls(
            primary_url=os.getenv("PRIMARY_URL", "http://localhost:8000").rstrip("/"),
            shard_url=os.getenv("SHARD_URL", "http://localhost:9091").rstrip("/"),
            gpu_threshold=_parse_float_env("GPU_THRESHOLD", 0.85),
            cb_error_threshold=_parse_float_env("CIRCUIT_BREAKER_ERROR_THRESHOLD", 0.05),
            cb_open_duration_seconds=_parse_float_env("CIRCUIT_BREAKER_OPEN_DURATION", 60.0),
            cb_window_seconds=_parse_float_env("CIRCUIT_BREAKER_WINDOW_SECONDS", 60.0),
            cb_half_open_probe_every=int(os.getenv("CIRCUIT_BREAKER_HALF_OPEN_EVERY", "10")),
            request_timeout_seconds=_parse_float_env("OVERFLOW_REQUEST_TIMEOUT_SECONDS", 120.0),
        )


class CircuitBreaker:
    """Tracks recent Shard failures and gates traffic to protect client traffic."""

    def __init__(
        self,
        error_threshold: float = 0.05,
        open_duration_seconds: float = 60.0,
        window_seconds: float = 60.0,
        half_open_probe_every: int = 10,
    ) -> None:
        self.error_threshold = error_threshold
        self.open_duration_seconds = open_duration_seconds
        self.window_seconds = window_seconds
        self.half_open_probe_every = max(1, half_open_probe_every)
        self._events: deque[tuple[float, bool]] = deque()
        self._state = "closed"
        self._state_changed_at = _now()
        self._half_open_counter = 0
        self._lock = asyncio.Lock()

    def _trim(self, now: float) -> None:
        cutoff = now - self.window_seconds
        while self._events and self._events[0][0] < cutoff:
            self._events.popleft()

    def _error_rate(self) -> float:
        if not self._events:
            return 0.0
        failures = sum(1 for _, ok in self._events if not ok)
        return failures / float(len(self._events))

    def _set_state(self, state: str, now: float) -> None:
        if self._state != state:
            self._state = state
            self._state_changed_at = now
            if state == "half_open":
                self._half_open_counter = 0
            log.warning(
                "overflow_circuit_breaker_state_change",
                state=state,
                changed_at=now,
            )

    async def state(self) -> str:
        async with self._lock:
            now = _now()
            self._trim(now)
            if self._state == "open" and (now - self._state_changed_at) >= self.open_duration_seconds:
                self._set_state("half_open", now)
            return self._state

    async def allow_shard_attempt(self) -> bool:
        async with self._lock:
            now = _now()
            self._trim(now)
            if self._state == "open":
                if (now - self._state_changed_at) >= self.open_duration_seconds:
                    self._set_state("half_open", now)
                else:
                    return False
            if self._state == "half_open":
                self._half_open_counter += 1
                return (self._half_open_counter % self.half_open_probe_every) == 0
            return True

    async def record_shard_result(self, ok: bool) -> None:
        async with self._lock:
            now = _now()
            self._trim(now)
            self._events.append((now, ok))
            error_rate = self._error_rate()
            if self._state == "half_open":
                if ok:
                    self._set_state("closed", now)
                else:
                    self._set_state("open", now)
                return
            if self._state == "closed" and error_rate > self.error_threshold:
                self._set_state("open", now)

    async def snapshot(self) -> dict[str, Any]:
        async with self._lock:
            now = _now()
            self._trim(now)
            state = self._state
            if state == "open" and (now - self._state_changed_at) >= self.open_duration_seconds:
                state = "half_open"
            total = len(self._events)
            failures = sum(1 for _, ok in self._events if not ok)
            return {
                "state": state,
                "window_total": total,
                "window_failures": failures,
                "window_error_rate": (failures / total) if total else 0.0,
                "state_changed_at": self._state_changed_at,
                "open_duration_seconds": self.open_duration_seconds,
            }


class SLAEnforcer:
    """
    Monitors p95 latency in a rolling window.
    If SLA is breached, temporarily disables Shard routing.
    """

    def __init__(
        self,
        p95_threshold_ms: float = 3000.0,
        window_size: int = 100,
        cooldown_seconds: float = 300.0,
    ) -> None:
        self.p95_threshold_ms = p95_threshold_ms
        self.window: deque[float] = deque(maxlen=max(1, window_size))
        self.cooldown_seconds = cooldown_seconds
        self._sla_breached_at: float | None = None
        self._breach_count = 0

    def p95(self) -> float:
        if not self.window:
            return 0.0
        ordered = sorted(self.window)
        idx = int(round((len(ordered) - 1) * 0.95))
        idx = max(0, min(idx, len(ordered) - 1))
        return float(ordered[idx])

    def record_latency(self, latency_ms: float) -> None:
        now = _now()
        self.window.append(float(latency_ms))
        if self.p95() > self.p95_threshold_ms:
            if self._sla_breached_at is None or (now - self._sla_breached_at) >= self.cooldown_seconds:
                self._breach_count += 1
            self._sla_breached_at = now

    def is_sla_healthy(self) -> bool:
        if self._sla_breached_at is None:
            return True
        return (_now() - self._sla_breached_at) >= self.cooldown_seconds

    def breach_count(self) -> int:
        return self._breach_count


class OverflowRouter:
    def __init__(self, config: RouterConfig) -> None:
        self.config = config
        self.client = httpx.AsyncClient(
            timeout=httpx.Timeout(config.request_timeout_seconds),
            limits=httpx.Limits(max_keepalive_connections=512, max_connections=1024),
        )
        self.circuit_breaker = CircuitBreaker(
            error_threshold=config.cb_error_threshold,
            open_duration_seconds=config.cb_open_duration_seconds,
            window_seconds=config.cb_window_seconds,
            half_open_probe_every=config.cb_half_open_probe_every,
        )
        self._metrics_lock = asyncio.Lock()
        self._requests_total: Counter[str] = Counter()
        self._gpu_utilization = 0.0
        self.sla_enforcer = SLAEnforcer(
            p95_threshold_ms=_parse_float_env("SLA_P95_THRESHOLD_MS", 3000.0),
            window_size=int(os.getenv("SLA_WINDOW_SIZE", "100")),
            cooldown_seconds=_parse_float_env("SLA_COOLDOWN_SECONDS", 300.0),
        )

    async def close(self) -> None:
        await self.client.aclose()

    async def get_gpu_utilization(self) -> float:
        # Prefer NVML if running on a GPU host.
        try:
            import pynvml  # type: ignore

            pynvml.nvmlInit()
            handle = pynvml.nvmlDeviceGetHandleByIndex(0)
            util = pynvml.nvmlDeviceGetUtilizationRates(handle)
            gpu = max(0.0, min(1.0, float(util.gpu) / 100.0))
            async with self._metrics_lock:
                self._gpu_utilization = gpu
            return gpu
        except Exception:
            pass

        # Fallback: read gpu_utilization_ratio from primary metrics endpoint.
        try:
            resp = await self.client.get(f"{self.config.primary_url}/metrics")
            if resp.is_success:
                for line in resp.text.splitlines():
                    if line.startswith("#"):
                        continue
                    if line.startswith("gpu_utilization_ratio"):
                        parts = line.strip().split()
                        if len(parts) >= 2:
                            gpu = max(0.0, min(1.0, float(parts[-1])))
                            async with self._metrics_lock:
                                self._gpu_utilization = gpu
                            return gpu
        except Exception:
            pass

        async with self._metrics_lock:
            return self._gpu_utilization

    @staticmethod
    def _copy_headers(request: Request) -> dict[str, str]:
        filtered = {}
        for k, v in request.headers.items():
            lower = k.lower()
            if lower in {"host", "content-length", "connection"}:
                continue
            filtered[k] = v
        return filtered

    async def _record_destination(self, destination: str) -> None:
        async with self._metrics_lock:
            self._requests_total[destination] += 1

    async def _proxy_non_streaming(
        self,
        destination_url: str,
        request: Request,
        body: bytes,
    ) -> Response:
        upstream = await self.client.request(
            request.method,
            destination_url,
            headers=self._copy_headers(request),
            content=body,
        )
        headers = {
            k: v
            for k, v in upstream.headers.items()
            if k.lower() not in {"transfer-encoding", "connection"}
        }
        return Response(
            content=upstream.content,
            status_code=upstream.status_code,
            headers=headers,
            media_type=upstream.headers.get("content-type"),
        )

    async def _proxy_streaming(
        self,
        destination_url: str,
        request: Request,
        body: bytes,
    ) -> StreamingResponse:
        stream_ctx = self.client.stream(
            request.method,
            destination_url,
            headers=self._copy_headers(request),
            content=body,
        )
        upstream = await stream_ctx.__aenter__()

        async def iterator() -> AsyncIterator[bytes]:
            try:
                async for chunk in upstream.aiter_bytes():
                    yield chunk
            finally:
                await stream_ctx.__aexit__(None, None, None)

        headers = {
            k: v
            for k, v in upstream.headers.items()
            if k.lower() not in {"transfer-encoding", "connection", "content-length"}
        }
        return StreamingResponse(
            iterator(),
            status_code=upstream.status_code,
            media_type=upstream.headers.get("content-type"),
            headers=headers,
        )

    async def _forward(
        self,
        destination: str,
        request: Request,
        body: bytes,
        stream: bool,
    ) -> Response:
        base = self.config.shard_url if destination == "shard" else self.config.primary_url
        destination_url = f"{base}/v1/chat/completions"
        if stream:
            return await self._proxy_streaming(destination_url, request, body)
        return await self._proxy_non_streaming(destination_url, request, body)

    @staticmethod
    def _is_stream_request(body: bytes) -> bool:
        try:
            payload = json.loads(body.decode("utf-8"))
        except Exception:
            return False
        return bool(payload.get("stream", False))

    async def route(self, request: Request) -> Response:
        body = await request.body()
        stream = self._is_stream_request(body)
        start = _now()
        gpu_util = await self.get_gpu_utilization()
        circuit_state = await self.circuit_breaker.state()

        destination = "primary"
        reason = "below_threshold"
        if circuit_state == "open":
            destination = "primary"
            reason = "circuit_open"
        elif not self.sla_enforcer.is_sla_healthy():
            destination = "primary"
            reason = "sla_breach_cooldown"
        elif gpu_util >= self.config.gpu_threshold:
            allowed = await self.circuit_breaker.allow_shard_attempt()
            if allowed:
                destination = "shard"
                reason = "above_threshold" if circuit_state == "closed" else "half_open_probe"
            else:
                destination = "primary"
                reason = "half_open_probe_skip"

        await self._record_destination(destination)
        log.info(
            "overflow_routing_decision",
            ts=start,
            routed_to=destination,
            gpu_util=round(gpu_util, 4),
            reason=reason,
            circuit_state=circuit_state,
        )

        try:
            response = await self._forward(destination, request, body, stream)
            if destination == "shard":
                ok = response.status_code < 500
                await self.circuit_breaker.record_shard_result(ok=ok)
                latency_ms = (_now() - start) * 1000.0
                self.sla_enforcer.record_latency(latency_ms)
                if not ok:
                    log.warning(
                        "overflow_shard_upstream_error",
                        status=response.status_code,
                        reason="shard_5xx",
                    )
            return response
        except Exception as exc:
            if destination == "shard":
                await self.circuit_breaker.record_shard_result(ok=False)
                log.warning("overflow_shard_request_failed", error=str(exc), fallback="primary")
                await self._record_destination("primary")
                return await self._forward("primary", request, body, stream)
            raise

    async def health(self) -> dict[str, Any]:
        snapshot = await self.circuit_breaker.snapshot()
        gpu_util = await self.get_gpu_utilization()
        return {
            "status": "ok",
            "gpu_utilization": round(gpu_util, 6),
            "gpu_threshold": self.config.gpu_threshold,
            "circuit_breaker_state": snapshot["state"],
            "circuit_breaker_window_error_rate": snapshot["window_error_rate"],
            "circuit_breaker_window_total": snapshot["window_total"],
            "sla_healthy": self.sla_enforcer.is_sla_healthy(),
            "sla_p95_ms": round(self.sla_enforcer.p95(), 3),
            "sla_breach_total": self.sla_enforcer.breach_count(),
        }

    async def prometheus_metrics(self) -> str:
        async with self._metrics_lock:
            counters = dict(self._requests_total)
            gpu = self._gpu_utilization
        cb = await self.circuit_breaker.snapshot()
        lines = [
            "# HELP overflow_requests_total Total number of chat requests routed by destination",
            "# TYPE overflow_requests_total counter",
            f'overflow_requests_total{{destination="primary"}} {counters.get("primary", 0)}',
            f'overflow_requests_total{{destination="shard"}} {counters.get("shard", 0)}',
            "# HELP overflow_gpu_utilization Current GPU utilization ratio sampled by overflow router",
            "# TYPE overflow_gpu_utilization gauge",
            f"overflow_gpu_utilization {gpu:.6f}",
            "# HELP overflow_circuit_breaker_state Circuit breaker state (1 for current state, 0 otherwise)",
            "# TYPE overflow_circuit_breaker_state gauge",
            f'overflow_circuit_breaker_state{{state="closed"}} {1 if cb["state"] == "closed" else 0}',
            f'overflow_circuit_breaker_state{{state="open"}} {1 if cb["state"] == "open" else 0}',
            f'overflow_circuit_breaker_state{{state="half_open"}} {1 if cb["state"] == "half_open" else 0}',
            "# HELP shard_sla_breach_total Number of SLA cooldown breaches recorded by overflow router",
            "# TYPE shard_sla_breach_total counter",
            f"shard_sla_breach_total {self.sla_enforcer.breach_count()}",
        ]
        return "\n".join(lines) + "\n"


config = RouterConfig.from_env()
router = OverflowRouter(config)
app = FastAPI(title="Shard Overflow Router")


@app.on_event("shutdown")
async def _shutdown() -> None:
    await router.close()


@app.get("/health")
async def health() -> JSONResponse:
    return JSONResponse(await router.health())


@app.get("/metrics")
async def metrics() -> Response:
    payload = await router.prometheus_metrics()
    return Response(content=payload, media_type="text/plain; version=0.0.4")


@app.post("/v1/chat/completions")
async def chat_completions(request: Request) -> Response:
    return await router.route(request)


if _parse_bool_env("OVERFLOW_SELF_TEST", default=False):
    log.info("overflow_router_loaded", config=config)
