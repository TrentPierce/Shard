from __future__ import annotations

import asyncio
import re
import statistics
from dataclasses import dataclass
from typing import Any

import httpx


@dataclass
class RequestMetric:
    draft_accepted: bool
    latency_ms: float
    tokens_in_draft: int
    error: str | None
    request_bytes: int
    response_bytes: int


class MetricsCollector:
    def __init__(self, base_url: str) -> None:
        self.base_url = base_url.rstrip("/")
        self._items: list[RequestMetric] = []
        self._lock = asyncio.Lock()
        self._metrics_before: dict[str, float] = {}
        self._metrics_after: dict[str, float] = {}

    async def capture_metrics_baseline(self) -> None:
        self._metrics_before = await self._fetch_prometheus_metrics()

    async def capture_metrics_after(self) -> None:
        self._metrics_after = await self._fetch_prometheus_metrics()

    async def _fetch_prometheus_metrics(self) -> dict[str, float]:
        try:
            async with httpx.AsyncClient(timeout=15.0) as client:
                resp = await client.get(f"{self.base_url}/metrics")
                resp.raise_for_status()
                return self._parse_prometheus_text(resp.text)
        except Exception:
            return {}

    @staticmethod
    def _parse_prometheus_text(text: str) -> dict[str, float]:
        parsed: dict[str, float] = {}
        line_re = re.compile(r"^([a-zA-Z_:][a-zA-Z0-9_:]*)(?:\{[^}]*\})?\s+(-?[0-9]+(?:\.[0-9]+)?)$")
        for raw in text.splitlines():
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            match = line_re.match(line)
            if not match:
                continue
            name = match.group(1)
            value = float(match.group(2))
            parsed[name] = value
        return parsed

    async def record(
        self,
        draft_accepted: bool,
        latency_ms: float,
        tokens_in_draft: int,
        error: str | None,
        request_bytes: int,
        response_bytes: int,
    ) -> None:
        metric = RequestMetric(
            draft_accepted=draft_accepted,
            latency_ms=latency_ms,
            tokens_in_draft=tokens_in_draft,
            error=error,
            request_bytes=request_bytes,
            response_bytes=response_bytes,
        )
        async with self._lock:
            self._items.append(metric)

    @staticmethod
    def _percentile(values: list[float], percentile: float) -> float:
        if not values:
            return 0.0
        ordered = sorted(values)
        if len(ordered) == 1:
            return ordered[0]
        idx = (len(ordered) - 1) * percentile
        lo = int(idx)
        hi = min(lo + 1, len(ordered) - 1)
        frac = idx - lo
        return ordered[lo] * (1.0 - frac) + ordered[hi] * frac

    def _network_overhead_per_request(self, total_requests: int) -> float:
        if total_requests == 0:
            return 0.0

        metric_candidates = [
            "shard_network_bytes_total",
            "shard_http_bytes_total",
            "process_network_receive_bytes_total",
            "process_network_transmit_bytes_total",
            "node_network_receive_bytes_total",
            "node_network_transmit_bytes_total",
        ]

        before_total = 0.0
        after_total = 0.0
        found_any = False
        for name in metric_candidates:
            if name in self._metrics_before and name in self._metrics_after:
                before_total += self._metrics_before[name]
                after_total += self._metrics_after[name]
                found_any = True

        if found_any:
            return max(0.0, (after_total - before_total) / total_requests)

        local_bytes = sum(item.request_bytes + item.response_bytes for item in self._items)
        return float(local_bytes) / float(total_requests)

    def summarize(
        self,
        baseline_latencies: list[float],
        scout_count: int,
        duration_seconds: int,
        base_url: str,
    ) -> dict[str, Any]:
        total_requests = len(self._items)
        errors = [item for item in self._items if item.error]
        accepted = [item for item in self._items if item.draft_accepted]
        latencies = [item.latency_ms for item in self._items]

        accepted_tokens = sum(item.tokens_in_draft for item in accepted)
        baseline_p95_ms = self._percentile([x * 1000.0 for x in baseline_latencies], 0.95)

        p50 = self._percentile(latencies, 0.5)
        p95 = self._percentile(latencies, 0.95)
        p99 = self._percentile(latencies, 0.99)
        latency_ratio = None
        if baseline_p95_ms > 0:
            latency_ratio = p95 / baseline_p95_ms

        total_errors = len(errors)
        error_rate_pct = (total_errors / total_requests * 100.0) if total_requests else 0.0
        acceptance_rate_pct = (len(accepted) / total_requests * 100.0) if total_requests else 0.0
        tokens_per_second = accepted_tokens / float(duration_seconds) if duration_seconds > 0 else 0.0
        network_overhead = self._network_overhead_per_request(total_requests)

        return {
            "timestamp_utc": __import__("datetime").datetime.utcnow().isoformat() + "Z",
            "base_url": base_url,
            "scout_count": scout_count,
            "duration_seconds": duration_seconds,
            "total_requests": total_requests,
            "total_errors": total_errors,
            "error_rate_pct": round(error_rate_pct, 4),
            "acceptance_rate_pct": round(acceptance_rate_pct, 4),
            "latency_p50_ms": round(p50, 3),
            "latency_p95_ms": round(p95, 3),
            "latency_p99_ms": round(p99, 3),
            "tokens_per_second": round(tokens_per_second, 3),
            "latency_vs_baseline_ratio": round(latency_ratio, 4) if latency_ratio is not None else None,
            "baseline_p95_ms": round(baseline_p95_ms, 3) if baseline_p95_ms else None,
            "network_overhead_bytes_per_request": round(network_overhead, 3),
            "mean_latency_ms": round(statistics.fmean(latencies), 3) if latencies else 0.0,
            "accepted_requests": len(accepted),
        }
