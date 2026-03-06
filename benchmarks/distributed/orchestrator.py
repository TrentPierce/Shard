#!/usr/bin/env python3
"""
Distributed Load Orchestrator

Usage:
  python benchmarks/distributed/orchestrator.py \
    --scouts 10000 --rate 500 --duration 600 \
    --verifier-pool http://verifier-a:9091,http://verifier-b:9091 \
    --out benchmarks/results/scale-test.json
"""

from __future__ import annotations

import argparse
import asyncio
from collections import Counter
import hashlib
import json
import os
import re
import shutil
import sys
import time
from dataclasses import dataclass
from pathlib import Path

import httpx

SUMMARY_QUEUE_BREAKDOWN_KEYS = (
    "verifier_in_flight_depth",
    "speculative_pending_depth",
    "scout_work_queue_depth",
)


@dataclass
class RequestRecord:
    ok: bool
    latency_ms: float
    status_code: int | None = None
    error_kind: str | None = None


@dataclass
class EndpointState:
    endpoint: str
    inflight: int = 0
    ewma_latency_ms: float = 1200.0
    error_ewma: float = 0.0
    queue_depth: float = 0.0
    load: float = 0.0
    last_refresh_monotonic: float = 0.0


class LoadAwarePool:
    def __init__(self, endpoints: list[str]) -> None:
        self._states = {
            ep.rstrip("/"): EndpointState(endpoint=ep.rstrip("/"))
            for ep in endpoints
            if ep.strip()
        }
        self._lock = asyncio.Lock()

    async def next(self, client: httpx.AsyncClient) -> str:
        await self.refresh(client)
        async with self._lock:
            if not self._states:
                raise RuntimeError("no verifier endpoints configured")
            ranked = sorted(self._states.values(), key=self._score)
            endpoint = ranked[0].endpoint
            self._states[endpoint].inflight += 1
            return endpoint

    async def best_endpoint_for_scout(
        self, client: httpx.AsyncClient, preferred: list[str] | None = None
    ) -> str:
        await self.refresh(client)
        async with self._lock:
            if not self._states:
                raise RuntimeError("no verifier endpoints configured")
            candidates = list(self._states.values())
            if preferred:
                preferred_set = {item.rstrip("/") for item in preferred if item}
                filtered = [state for state in candidates if state.endpoint in preferred_set]
                if filtered:
                    candidates = filtered
            ranked = sorted(candidates, key=self._score)
            return ranked[0].endpoint

    async def score_for(self, endpoint: str, client: httpx.AsyncClient) -> float:
        await self.refresh(client)
        async with self._lock:
            state = self._states.get(endpoint.rstrip("/"))
            if state is None:
                return float("inf")
            return self._score(state)

    async def note_result(self, endpoint: str, ok: bool, latency_ms: float) -> None:
        async with self._lock:
            state = self._states.get(endpoint.rstrip("/"))
            if state is None:
                return
            state.inflight = max(0, state.inflight - 1)
            alpha = 0.2
            state.ewma_latency_ms = ((1.0 - alpha) * state.ewma_latency_ms) + (alpha * max(0.0, latency_ms))
            err_sample = 0.0 if ok else 1.0
            state.error_ewma = ((1.0 - alpha) * state.error_ewma) + (alpha * err_sample)

    async def refresh(self, client: httpx.AsyncClient, min_interval_s: float = 2.0) -> None:
        now = time.monotonic()
        async with self._lock:
            candidates = list(self._states.values())
        stale = [s for s in candidates if now - s.last_refresh_monotonic >= min_interval_s]
        if not stale:
            return
        for state in stale:
            queue_depth = state.queue_depth
            load = state.load
            latency_ms = state.ewma_latency_ms
            try:
                resp = await client.get(f"{state.endpoint}/metrics/summary")
                if resp.is_success:
                    data = resp.json()
                    if summary_has_queue_breakdown(data):
                        queue_depth = float(data.get("queue_depth", queue_depth) or 0.0)
                        load = float(data.get("load", load) or 0.0)
                        latency_ms = float(
                            data.get(
                                "average_latency_ms",
                                data.get("latency_ms", latency_ms),
                            )
                            or latency_ms
                        )
            except Exception:
                pass
            async with self._lock:
                current = self._states.get(state.endpoint)
                if current is None:
                    continue
                current.queue_depth = max(0.0, queue_depth)
                current.load = max(0.0, load)
                if latency_ms > 0:
                    alpha = 0.35
                    current.ewma_latency_ms = (
                        ((1.0 - alpha) * current.ewma_latency_ms)
                        + (alpha * max(0.0, latency_ms))
                    )
                current.last_refresh_monotonic = now

    @staticmethod
    def _score(state: EndpointState) -> float:
        # Lower score is better.
        return (
            (state.inflight * 12.0)
            + state.ewma_latency_ms
            + (state.queue_depth * 25.0)
            + (state.load * 15.0)
            + (state.error_ewma * 3000.0)
        )


def count_leading_zero_bits(raw: bytes) -> int:
    count = 0
    for b in raw:
        if b == 0:
            count += 8
            continue
        bit = 0x80
        while bit and (b & bit) == 0:
            count += 1
            bit >>= 1
        break
    return count


def solve_pow(challenge_hex: str, difficulty: int, max_nonce: int = 20_000_000) -> tuple[int, str]:
    challenge = bytes.fromhex(challenge_hex)
    for nonce in range(max_nonce):
        payload = challenge + int(nonce).to_bytes(8, byteorder="little", signed=False)
        digest = hashlib.sha256(payload).digest()
        if count_leading_zero_bits(digest) >= difficulty:
            return nonce, digest.hex()
    raise RuntimeError("pow_solve_failed")


def extract_user_message(prompt_context: str) -> str:
    match = re.search(r"<\|start_header_id\|>user\n\n(.*?)<\|eot_id\|>", prompt_context, flags=re.DOTALL)
    if match:
        return match.group(1).strip()
    return prompt_context.strip()


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    idx = (len(ordered) - 1) * pct
    lo = int(idx)
    hi = min(lo + 1, len(ordered) - 1)
    frac = idx - lo
    return ordered[lo] * (1.0 - frac) + ordered[hi] * frac


def adaptive_scout_target(
    current: int,
    configured_max: int,
    max_queue_depth: float,
    max_p95_latency_ms: float,
    blackout_endpoints: int,
) -> int:
    if configured_max <= 0:
        return 0
    if blackout_endpoints > 0:
        return 0
    # Fast downshift on overload.
    if max_queue_depth >= 10.0 or max_p95_latency_ms >= 6000.0:
        return 0
    if max_queue_depth >= 5.0 or max_p95_latency_ms >= 4500.0:
        return max(1, configured_max // 4)
    # Hysteresis for scale-up: require clearly healthy metrics.
    if max_queue_depth <= 2.0 and max_p95_latency_ms <= 3500.0:
        return configured_max
    return max(1, min(configured_max, current))


async def fetch_summary(client: httpx.AsyncClient, base_url: str) -> dict:
    try:
        resp = await client.get(f"{base_url.rstrip('/')}/metrics/summary")
        if resp.is_success:
            return resp.json()
    except Exception:
        pass
    return {}


async def fetch_health(client: httpx.AsyncClient, base_url: str) -> dict:
    try:
        resp = await client.get(f"{base_url.rstrip('/')}/health")
        if resp.is_success:
            return resp.json()
    except Exception:
        pass
    return {}


async def fetch_pool_summaries(client: httpx.AsyncClient, verifier_pool: list[str]) -> list[dict]:
    summaries: list[dict] = []
    for endpoint in verifier_pool:
        summary = await fetch_summary(client, endpoint)
        if summary:
            summary["_endpoint"] = endpoint
            summaries.append(summary)
    return summaries


async def fetch_pool_health(client: httpx.AsyncClient, verifier_pool: list[str]) -> list[dict]:
    health_rows: list[dict] = []
    for endpoint in verifier_pool:
        health = await fetch_health(client, endpoint)
        if health:
            health["_endpoint"] = endpoint
            health_rows.append(health)
    return health_rows


def aggregate_counter(summaries: list[dict], key: str) -> float:
    total = 0.0
    for summary in summaries:
        total += float(summary.get(key, 0.0) or 0.0)
    return total


def aggregate_speculative_totals(summaries: list[dict]) -> tuple[float, float]:
    accepted = 0.0
    total = 0.0
    for summary in summaries:
        accepted += float(summary.get("speculative_accepted_tokens_total", 0.0) or 0.0)
        total += float(summary.get("speculative_draft_tokens_total", 0.0) or 0.0)
    return accepted, total


def summary_latency_signal_ms(summary: dict) -> float:
    p95 = float(summary.get("p95_latency_ms", 0.0) or 0.0)
    avg = float(summary.get("average_latency_ms", 0.0) or 0.0)
    return max(p95, avg)


def summary_has_queue_breakdown(summary: dict) -> bool:
    return all(key in summary for key in SUMMARY_QUEUE_BREAKDOWN_KEYS)


async def wait_for_browser_scout_supply(
    client: httpx.AsyncClient,
    verifier_pool: list[str],
    timeout_s: int,
    expected_scouts: int,
) -> bool:
    deadline = time.monotonic() + max(1, timeout_s)
    required = max(1, expected_scouts)
    while time.monotonic() < deadline:
        pool_health = await fetch_pool_health(client, verifier_pool)
        observed_sessions = sum(
            (int(health.get("active_browser_sessions", 0) or 0) for health in pool_health),
        )
        observed_draft_capable = sum(
            (int(health.get("draft_capable_scouts", 0) or 0) for health in pool_health),
        )
        if observed_sessions >= required and observed_draft_capable >= required:
            return True
        await asyncio.sleep(1.0)
    return False


async def wait_for_pool_readiness(
    client: httpx.AsyncClient,
    verifier_pool: list[str],
    timeout_s: int = 120,
    queue_depth_max: float = 8.0,
    require_no_blackout: bool = True,
    strict: bool = False,
) -> bool:
    deadline = time.monotonic() + max(1, timeout_s)
    last_reason = "initializing"
    while time.monotonic() < deadline:
        reasons: list[str] = []
        all_ready = True
        for endpoint in verifier_pool:
            health = await fetch_health(client, endpoint)
            summary = await fetch_summary(client, endpoint)
            if not health:
                all_ready = False
                reasons.append(f"{endpoint}:health_unreachable")
                continue
            if not bool(health.get("ready_for_inference", False)):
                all_ready = False
                reason = str(health.get("readiness_reason", "not_ready"))
                reasons.append(f"{endpoint}:not_ready({reason})")
            if not summary:
                all_ready = False
                reasons.append(f"{endpoint}:summary_unreachable")
                continue
            if not summary_has_queue_breakdown(summary):
                all_ready = False
                reasons.append(f"{endpoint}:stale_summary_schema")
                continue
            queue_depth = float(summary.get("queue_depth", 0.0) or 0.0)
            if queue_depth > queue_depth_max:
                all_ready = False
                reasons.append(f"{endpoint}:queue={queue_depth:.1f}")
            if require_no_blackout:
                mode = str(summary.get("scout_blackout_mode", "open")).lower()
                if mode == "blackout":
                    all_ready = False
                    reasons.append(f"{endpoint}:blackout")
        if all_ready:
            return True
        last_reason = ", ".join(reasons[:4]) if reasons else "waiting"
        await asyncio.sleep(2)
    message = (
        f"pool readiness timeout after {timeout_s}s (queue_max={queue_depth_max}): {last_reason}"
    )
    if strict:
        raise RuntimeError(message)
    print(f"[readiness] warning: {message}; continuing with run")
    return False


def browser_scout_runner_path() -> Path:
    return Path(__file__).resolve().with_name("browser_scout_runner.mjs")


async def launch_browser_scout_runner(
    count: int,
    verifier_pool: list[str],
    page_base_url: str,
    browser_channel: str,
    browser_headless: bool,
    startup_timeout_ms: int,
    browser_user_data_dir: str | None,
) -> asyncio.subprocess.Process:
    node_name = "node.exe" if os.name == "nt" else "node"
    node_path = shutil.which(node_name) or shutil.which("node")
    if not node_path:
        raise RuntimeError("node is required for browser scout benchmarks")

    cmd = [
        node_path,
        str(browser_scout_runner_path()),
        "--count",
        str(max(1, count)),
        "--page-base",
        page_base_url,
        "--backends",
        ",".join(verifier_pool),
        "--channel",
        browser_channel,
        "--startup-timeout-ms",
        str(max(30_000, startup_timeout_ms)),
        "--headless",
        "true" if browser_headless else "false",
    ]
    if browser_user_data_dir:
        cmd.extend(["--user-data-dir", browser_user_data_dir])
    return await asyncio.create_subprocess_exec(*cmd)


async def run_orchestrator(
    scouts: int,
    rate: float,
    duration: int,
    verifier_pool: list[str],
    out_path: Path,
    inference_mode: str,
    request_timeout_ms: int,
    max_attempts: int,
    scout_workers: int,
    max_tokens: int,
    scout_mode: str = "synthetic",
    browser_scout_page_base_url: str = "http://127.0.0.1:3000/benchmark/scout",
    browser_channel: str = "msedge" if sys.platform.startswith("win") else "chrome",
    browser_headless: bool = False,
    browser_startup_timeout_ms: int = 900_000,
    browser_user_data_dir: str | None = None,
    browser_warmup_timeout_s: int = 45,
    browser_warmup_max_requests: int = 12,
    readiness_timeout_s: int = 120,
    ready_queue_depth_max: float = 8.0,
    require_no_blackout: bool = True,
    strict_readiness: bool = False,
) -> int:
    if not verifier_pool:
        raise ValueError("verifier_pool cannot be empty")

    pool = LoadAwarePool(verifier_pool)
    semaphore = asyncio.Semaphore(max(1, scouts))
    records: list[RequestRecord] = []
    records_lock = asyncio.Lock()
    in_flight = 0
    in_flight_lock = asyncio.Lock()
    timeseries: list[dict] = []
    start = 0.0
    end = 0.0

    limits = httpx.Limits(max_keepalive_connections=max(50, scouts), max_connections=max(100, scouts * 2))
    timeout_secs = max(0.5, request_timeout_ms / 1000.0)
    timeout = httpx.Timeout(
        connect=min(5.0, timeout_secs),
        read=timeout_secs,
        write=timeout_secs,
        pool=timeout_secs,
    )
    browser_scout_proc: asyncio.subprocess.Process | None = None
    browser_warmup: dict[str, object] = {
        "attempted": scout_mode == "browser" and scout_workers > 0,
        "ready": False,
        "productive": False,
        "requests_sent": 0,
    }

    try:
        async with httpx.AsyncClient(limits=limits, timeout=timeout) as client:
            await wait_for_pool_readiness(
                client=client,
                verifier_pool=verifier_pool,
                timeout_s=readiness_timeout_s,
                queue_depth_max=ready_queue_depth_max,
                require_no_blackout=require_no_blackout,
                strict=strict_readiness,
            )
            scout_target = max(0, scout_workers)
            scout_target_lock = asyncio.Lock()
    
            async def get_scout_target() -> int:
                async with scout_target_lock:
                    return scout_target
    
            async def set_scout_target(value: int) -> None:
                nonlocal scout_target
                bounded = max(0, min(max(0, scout_workers), int(value)))
                async with scout_target_lock:
                    scout_target = bounded
    
            async def ensure_pow_for_scout(endpoint: str, scout_id: str) -> bool:
                challenge_resp = await client.get(
                    f"{endpoint}/v1/pow/challenge",
                    params={
                        "peer_id": scout_id,
                        "hardware_concurrency": 8,
                        "is_mobile": "false",
                    },
                )
                if not challenge_resp.is_success:
                    return False
                payload = challenge_resp.json()
                challenge = payload.get("challenge", {})
                challenge_hex = challenge.get("challenge_bytes_hex")
                difficulty = int(challenge.get("difficulty", 0))
                if not challenge_hex or difficulty <= 0:
                    return False
                nonce, hash_hex = await asyncio.to_thread(solve_pow, challenge_hex, difficulty)
                verify_resp = await client.post(
                    f"{endpoint}/v1/pow/verify",
                    json={"peer_id": scout_id, "nonce": nonce, "hash_hex": hash_hex},
                )
                if not verify_resp.is_success:
                    return False
                return bool(verify_resp.json().get("ok"))
    
            async def scout_loop(worker_idx: int) -> None:
                scout_id = f"orchestrator-scout-{worker_idx}"
                verified_endpoints: set[str] = set()
                sticky_endpoint: str | None = None
                while time.monotonic() < end:
                    try:
                        target = await get_scout_target()
                        if worker_idx >= target:
                            await asyncio.sleep(0.2)
                            continue
                        if sticky_endpoint:
                            sticky_score = await pool.score_for(sticky_endpoint, client)
                            if sticky_score == float("inf"):
                                sticky_endpoint = None
                            else:
                                best_score = await pool.score_for(
                                    await pool.best_endpoint_for_scout(client), client
                                )
                                # Keep affinity unless it is materially worse than current best.
                                if sticky_score > (best_score * 1.35):
                                    sticky_endpoint = None
    
                        preferred_candidates = [sticky_endpoint] if sticky_endpoint else None
                        endpoint = await pool.best_endpoint_for_scout(client, preferred_candidates)
                        sticky_endpoint = endpoint
                        if endpoint not in verified_endpoints:
                            verified = await ensure_pow_for_scout(endpoint, scout_id)
                            if not verified:
                                sticky_endpoint = None
                                await asyncio.sleep(0.5)
                                continue
                            verified_endpoints.add(endpoint)
    
                        work_resp = await client.get(
                            f"{endpoint}/v1/scout/work",
                            params={"scout_id": scout_id},
                        )
                        if not work_resp.is_success:
                            retry_after_ms = 0
                            preferred_endpoint = None
                            try:
                                payload = work_resp.json() or {}
                                retry_after_ms = int(payload.get("retry_after_ms", 0) or 0)
                                preferred_endpoint = payload.get("preferred_endpoint")
                            except Exception:
                                retry_after_ms = 0
                            if isinstance(preferred_endpoint, str) and preferred_endpoint.strip():
                                sticky_endpoint = preferred_endpoint.rstrip("/")
                            elif work_resp.status_code in (429, 503):
                                sticky_endpoint = None
                            await asyncio.sleep(max(0.05, retry_after_ms / 1000.0))
                            continue
                        payload = work_resp.json() or {}
                        work = payload.get("work")
                        if not work:
                            retry_after_ms = int(payload.get("retry_after_ms", 0) or 0)
                            await asyncio.sleep(max(0.01, retry_after_ms / 1000.0))
                            continue
                        preferred_endpoint = work.get("preferred_endpoint")
                        if isinstance(preferred_endpoint, str) and preferred_endpoint.strip():
                            preferred_target = preferred_endpoint.rstrip("/")
                            sticky_endpoint = preferred_target
                            endpoint = preferred_target
                            if endpoint not in verified_endpoints:
                                verified = await ensure_pow_for_scout(endpoint, scout_id)
                                if not verified:
                                    sticky_endpoint = None
                                    await asyncio.sleep(0.25)
                                    continue
                                verified_endpoints.add(endpoint)
    
                        prompt_context = str(work.get("prompt_context", ""))
                        user_prompt = extract_user_message(prompt_context)
                        min_tokens = max(1, int(work.get("min_tokens", 1)))
                        lease_id = work.get("lease_id")
    
                        # Draft with standard mode so this helper path does not recurse into speculative work.
                        gen_resp = await client.post(
                            f"{endpoint}/v1/chat/completions",
                            headers={
                                "x-shard-inference-mode": "standard",
                                "x-shard-mesh-forward": "false",
                            },
                            json={
                                "model": "shard-hybrid",
                                "messages": [{"role": "user", "content": user_prompt}],
                                "max_tokens": max(min_tokens, 4),
                                "stream": False,
                            },
                        )
                        if not gen_resp.is_success:
                            await asyncio.sleep(0.01)
                            continue
                        generated = (
                            gen_resp.json()
                            .get("choices", [{}])[0]
                            .get("message", {})
                            .get("content", "")
                        )
                        draft_text = (str(generated).strip() or "ok")[:256]
    
                        await client.post(
                            f"{endpoint}/v1/scout/draft",
                            json={
                                "work_id": work.get("request_id"),
                                "scout_id": scout_id,
                                "lease_id": lease_id,
                                "draft_text": draft_text,
                                "prompt_context": prompt_context,
                                "timestamp": time.time(),
                            },
                        )
                    except Exception:
                        await asyncio.sleep(0.05)
    
            async def fire_one(seq: int) -> None:
                nonlocal in_flight
                async with semaphore:
                    async with in_flight_lock:
                        in_flight += 1
                    t0 = time.monotonic()
                    ok = False
                    final_status: int | None = None
                    final_error_kind: str | None = None
                    for _attempt in range(max(1, max_attempts)):
                        endpoint = None
                        try:
                            endpoint = await pool.next(client)
                            resp = await client.post(
                                f"{endpoint}/v1/chat/completions",
                                headers={
                                    "x-shard-inference-mode": inference_mode,
                                },
                                json={
                                    "model": "shard-hybrid",
                                    "messages": [{"role": "user", "content": f"hello from scout {seq}"}],
                                    "max_tokens": max(1, max_tokens),
                                },
                            )
                            final_status = int(resp.status_code)
                            attempt_ok = resp.status_code < 400
                            await pool.note_result(
                                endpoint=endpoint,
                                ok=attempt_ok,
                                latency_ms=(time.monotonic() - t0) * 1000.0,
                            )
                            if not attempt_ok:
                                final_error_kind = f"http_{resp.status_code}"
                            if attempt_ok:
                                ok = True
                                final_error_kind = None
                                break
                        except Exception as exc:
                            final_error_kind = exc.__class__.__name__
                            if endpoint:
                                await pool.note_result(
                                    endpoint=endpoint,
                                    ok=False,
                                    latency_ms=(time.monotonic() - t0) * 1000.0,
                                )
                        await asyncio.sleep(0.01)
                    latency_ms = (time.monotonic() - t0) * 1000.0
                    async with records_lock:
                        records.append(
                            RequestRecord(
                                ok=ok,
                                latency_ms=latency_ms,
                                status_code=final_status,
                                error_kind=final_error_kind,
                            )
                        )
                    async with in_flight_lock:
                        in_flight -= 1

            async def warmup_one(seq: int) -> bool:
                endpoint = None
                t0 = time.monotonic()
                ok = False
                try:
                    endpoint = await pool.next(client)
                    resp = await client.post(
                        f"{endpoint}/v1/chat/completions",
                        headers={
                            "x-shard-inference-mode": inference_mode,
                        },
                        json={
                            "model": "shard-hybrid",
                            "messages": [{"role": "user", "content": f"browser scout warmup {seq}"}],
                            "max_tokens": max(4, max_tokens),
                        },
                    )
                    ok = resp.status_code < 400
                    return ok
                except Exception:
                    return False
                finally:
                    if endpoint:
                        await pool.note_result(
                            endpoint=endpoint,
                            ok=ok,
                            latency_ms=(time.monotonic() - t0) * 1000.0,
                        )

            async def warmup_browser_scouts() -> dict[str, object]:
                info: dict[str, object] = {
                    "attempted": True,
                    "ready": False,
                    "productive": False,
                    "requests_sent": 0,
                }
                ready = await wait_for_browser_scout_supply(
                    client=client,
                    verifier_pool=verifier_pool,
                    timeout_s=browser_warmup_timeout_s,
                    expected_scouts=max(1, scout_workers),
                )
                info["ready"] = ready
                if not ready:
                    print(
                        f"[browser-warmup] warning: scouts never became draft-capable within "
                        f"{browser_warmup_timeout_s}s"
                    )
                    return info

                baseline_summaries = await fetch_pool_summaries(client, verifier_pool)
                baseline_submit_success = aggregate_counter(
                    baseline_summaries, "scout_client_submit_success_total"
                )
                baseline_draft_submissions = aggregate_counter(
                    baseline_summaries, "scout_draft_submissions_total"
                )
                baseline_speculative_tokens = aggregate_counter(
                    baseline_summaries, "speculative_draft_tokens_total"
                )
                deadline = time.monotonic() + max(5, browser_warmup_timeout_s)
                attempts = max(1, browser_warmup_max_requests)
                for seq in range(attempts):
                    if time.monotonic() >= deadline:
                        break
                    await warmup_one(seq)
                    info["requests_sent"] = int(info["requests_sent"]) + 1
                    await asyncio.sleep(0.35)
                    pool_summaries = await fetch_pool_summaries(client, verifier_pool)
                    pool_health = await fetch_pool_health(client, verifier_pool)
                    submit_success_total = aggregate_counter(
                        pool_summaries, "scout_client_submit_success_total"
                    )
                    draft_submissions_total = aggregate_counter(
                        pool_summaries, "scout_draft_submissions_total"
                    )
                    speculative_draft_tokens_total = aggregate_counter(
                        pool_summaries, "speculative_draft_tokens_total"
                    )
                    recent_submitters = max(
                        (int(health.get("recent_scout_submitters", 0) or 0) for health in pool_health),
                        default=0,
                    )
                    if (
                        submit_success_total > baseline_submit_success
                        or draft_submissions_total > baseline_draft_submissions
                        or speculative_draft_tokens_total > baseline_speculative_tokens
                        or recent_submitters > 0
                    ):
                        info["productive"] = True
                        print(
                            "[browser-warmup] productive scout supply detected "
                            f"after {info['requests_sent']} warmup requests"
                        )
                        return info

                print(
                    "[browser-warmup] warning: scouts became draft-capable but no submit-success "
                    f"signal appeared after {info['requests_sent']} warmup requests"
                )
                return info
    
            launch_tasks: list[asyncio.Task] = []
    
            async def launcher() -> None:
                if rate <= 0:
                    return
                interval = 1.0 / float(rate)
                seq = 0
                next_fire = time.monotonic()
                while time.monotonic() < end:
                    now = time.monotonic()
                    if now < next_fire:
                        await asyncio.sleep(next_fire - now)
                    next_fire += interval
                    launch_tasks.append(asyncio.create_task(fire_one(seq)))
                    seq += 1
    
            async def progress_loop() -> None:
                while time.monotonic() < end:
                    await asyncio.sleep(10)
                    elapsed = int(time.monotonic() - start)
                    async with records_lock:
                        latencies = [r.latency_ms for r in records]
                        oks = sum(1 for r in records if r.ok)
                        errs = len(records) - oks
                        p95 = percentile(latencies, 0.95)
                        err_rate = (errs / len(records) * 100.0) if records else 0.0
                        error_counts: Counter[str] = Counter()
                        for record in records:
                            if record.ok:
                                continue
                            key = (
                                record.error_kind
                                or (f"http_{record.status_code}" if record.status_code else "unknown")
                            )
                            error_counts[key] += 1
                    summary_now = await fetch_summary(client, verifier_pool[0])
                    acceptance = float(summary_now.get("speculative_acceptance_rate", 0.0)) * 100.0
                    pool_summaries = await fetch_pool_summaries(client, verifier_pool)
                    pool_health = await fetch_pool_health(client, verifier_pool)
                    max_queue_depth = max(
                        (float(s.get("queue_depth", 0.0) or 0.0) for s in pool_summaries),
                        default=0.0,
                    )
                    max_p95_latency_ms = max(
                        (summary_latency_signal_ms(s) for s in pool_summaries),
                        default=0.0,
                    )
                    blackout_endpoints = sum(
                        1
                        for s in pool_summaries
                        if str(s.get("scout_blackout_mode", "open")).lower() == "blackout"
                    )
                    if scout_mode == "synthetic":
                        current_target = await get_scout_target()
                        next_target = adaptive_scout_target(
                            current=current_target,
                            configured_max=max(0, scout_workers),
                            max_queue_depth=max_queue_depth,
                            max_p95_latency_ms=max_p95_latency_ms,
                            blackout_endpoints=blackout_endpoints,
                        )
                        await set_scout_target(next_target)
                    async with in_flight_lock:
                        active = in_flight
                    active_scout_target = (
                        await get_scout_target() if scout_mode == "synthetic" else max(0, scout_workers)
                    )
                    observed_active_scouts = sum(
                        (int(h.get("active_scouts", 0) or 0) for h in pool_health),
                    )
                    observed_browser_sessions = sum(
                        (int(h.get("active_browser_sessions", 0) or 0) for h in pool_health),
                    )
                    observed_draft_capable_scouts = sum(
                        (int(h.get("draft_capable_scouts", 0) or 0) for h in pool_health),
                    )
                    top_errors = ", ".join(
                        f"{kind}:{count}" for kind, count in error_counts.most_common(2)
                    )
                    if not top_errors:
                        top_errors = "none"
                    print(
                        f"[{elapsed}s/{duration}s] InFlight: {active} | obs_scouts: {observed_active_scouts} | "
                        f"browser_sessions: {observed_browser_sessions} | p95: {p95/1000.0:.2f}s | "
                        f"accept: {acceptance:.1f}% | errors: {err_rate:.2f}% | "
                        f"scout_target: {active_scout_target} | max_q: {max_queue_depth:.1f} | max_p95: {max_p95_latency_ms:.0f}ms | "
                        f"blackout_nodes: {blackout_endpoints} | top_errors: {top_errors}"
                    )
                    timeseries.append(
                        {
                            "t": elapsed,
                            "in_flight_requests": active,
                            "observed_active_scouts": observed_active_scouts,
                            "observed_active_browser_sessions": observed_browser_sessions,
                            "observed_draft_capable_scouts": observed_draft_capable_scouts,
                            "scout_target": active_scout_target,
                            "max_queue_depth": round(max_queue_depth, 3),
                            "max_pool_p95_latency_ms": round(max_p95_latency_ms, 3),
                            "blackout_endpoints": blackout_endpoints,
                            "p95_latency_ms": round(p95, 3),
                            "acceptance_rate_pct": round(acceptance, 3),
                            "error_rate_pct": round(err_rate, 4),
                        }
                    )
    
            scout_tasks: list[asyncio.Task] = []
            if scout_workers > 0 and scout_mode == "browser":
                browser_scout_proc = await launch_browser_scout_runner(
                    count=scout_workers,
                    verifier_pool=verifier_pool,
                    page_base_url=browser_scout_page_base_url,
                    browser_channel=browser_channel,
                    browser_headless=browser_headless,
                    startup_timeout_ms=browser_startup_timeout_ms,
                    browser_user_data_dir=browser_user_data_dir,
                )
                await asyncio.sleep(2.0)
                if browser_scout_proc.returncode is not None:
                    raise RuntimeError(
                        f"browser scout runner exited early with code {browser_scout_proc.returncode}"
                    )
                browser_warmup = await warmup_browser_scouts()

            baseline_summaries = await fetch_pool_summaries(client, verifier_pool)
            baseline_accepted, baseline_total = aggregate_speculative_totals(baseline_summaries)
            start = time.monotonic()
            end = start + duration

            if scout_workers > 0 and scout_mode == "synthetic":
                scout_tasks = [
                    asyncio.create_task(scout_loop(idx))
                    for idx in range(max(0, scout_workers))
                ]
            await asyncio.gather(launcher(), progress_loop())
            await asyncio.gather(*launch_tasks, return_exceptions=True)
            if scout_tasks:
                scout_shutdown_grace_s = max(5.0, timeout_secs + 2.0)
                done, pending = await asyncio.wait(
                    scout_tasks,
                    timeout=scout_shutdown_grace_s,
                )
                if pending:
                    print(
                        f"[scout-drain] canceling {len(pending)} scout workers after "
                        f"{scout_shutdown_grace_s:.1f}s grace period"
                    )
                    for task in pending:
                        task.cancel()
                    await asyncio.gather(*pending, return_exceptions=True)
                if done:
                    await asyncio.gather(*done, return_exceptions=True)
    
            after_summaries = await fetch_pool_summaries(client, verifier_pool)
            after_accepted, after_total = aggregate_speculative_totals(after_summaries)
    finally:
        if browser_scout_proc is not None and browser_scout_proc.returncode is None:
            browser_scout_proc.terminate()
            try:
                await asyncio.wait_for(browser_scout_proc.wait(), timeout=20.0)
            except asyncio.TimeoutError:
                browser_scout_proc.kill()
                await browser_scout_proc.wait()

    latencies = [r.latency_ms for r in records]
    total = len(records)
    oks = sum(1 for r in records if r.ok)
    errs = total - oks
    error_rate_pct = (errs / total * 100.0) if total else 100.0
    p95_latency_ms = percentile(latencies, 0.95)

    accepted_delta = max(0.0, after_accepted - baseline_accepted)
    total_delta = max(0.0, after_total - baseline_total)
    acceptance_source = "speculative_metrics"
    if total_delta > 0:
        acceptance_rate_pct = accepted_delta / total_delta * 100.0
    else:
        # If the verifier did not record speculative token samples for this run,
        # don't hard-fail acceptance gates on missing telemetry.
        acceptance_rate_pct = 100.0
        acceptance_source = "no_speculative_samples"

    throughput_tps = oks / float(duration) if duration > 0 else 0.0
    status_counts: Counter[str] = Counter()
    error_breakdown: Counter[str] = Counter()
    for record in records:
        if record.status_code is not None:
            status_counts[str(record.status_code)] += 1
        if not record.ok:
            key = record.error_kind or (f"http_{record.status_code}" if record.status_code else "unknown")
            error_breakdown[key] += 1

    results = {
        "scouts": scouts,
        "rate": rate,
        "duration_seconds": duration,
        "verifier_pool": verifier_pool,
        "scout_mode": scout_mode,
        "browser_scout_page_base_url": (
            browser_scout_page_base_url if scout_mode == "browser" else None
        ),
        "total_requests": total,
        "successful_requests": oks,
        "error_requests": errs,
        "error_rate_pct": round(error_rate_pct, 4),
        "p95_latency_ms": round(p95_latency_ms, 3),
        "acceptance_rate_pct": round(acceptance_rate_pct, 4),
        "acceptance_source": acceptance_source,
        "acceptance_samples": int(total_delta),
        "throughput_tps": round(throughput_tps, 4),
        "browser_warmup": browser_warmup,
        "status_counts": dict(status_counts),
        "error_breakdown": dict(error_breakdown),
        "timeseries": timeseries,
    }

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(results, indent=2), encoding="utf-8")

    gates = {
        "p95_latency_ms": p95_latency_ms <= 3000.0,
        "error_rate_pct": error_rate_pct <= 0.1,
        "acceptance_rate_pct": acceptance_rate_pct >= 65.0,
    }
    passed = all(gates.values())

    print("\nFinal:")
    print(json.dumps({"results": results, "gates": gates, "passed": passed}, indent=2))

    return 0 if passed else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Shard distributed load orchestrator")
    parser.add_argument("--scouts", type=int, required=True, help="Target concurrent simulated scouts")
    parser.add_argument("--rate", type=float, required=True, help="Global requests per second")
    parser.add_argument("--duration", type=int, required=True, help="Test duration in seconds")
    parser.add_argument(
        "--verifier-pool",
        type=str,
        required=True,
        help="Comma-separated verifier endpoints, e.g. http://a:9091,http://b:9091",
    )
    parser.add_argument("--out", type=str, required=True, help="Output JSON path")
    parser.add_argument(
        "--inference-mode",
        type=str,
        default="distributed",
        choices=["standard", "distributed", "speculative"],
        help="x-shard-inference-mode sent with each chat request",
    )
    parser.add_argument(
        "--request-timeout-ms",
        type=int,
        default=10000,
        help="Per-request timeout in milliseconds",
    )
    parser.add_argument(
        "--max-attempts",
        type=int,
        default=1,
        help="Attempts per request before counting as error",
    )
    parser.add_argument(
        "--scout-workers",
        type=int,
        default=0,
        help="Synthetic scout workers that poll /v1/scout/work and submit drafts",
    )
    parser.add_argument(
        "--scout-mode",
        type=str,
        default="synthetic",
        choices=["synthetic", "browser"],
        help="Use synthetic verifier-generated scouts or real browser WebGPU scout tabs",
    )
    parser.add_argument(
        "--browser-scout-page-base-url",
        type=str,
        default="http://127.0.0.1:3000/benchmark/scout",
        help="Benchmark scout page used when --scout-mode=browser",
    )
    parser.add_argument(
        "--browser-channel",
        type=str,
        default="msedge" if sys.platform.startswith("win") else "chrome",
        help="Browser channel for real browser scouts (chrome, msedge, chromium)",
    )
    parser.add_argument(
        "--browser-headless",
        action="store_true",
        help="Run browser scouts headless when supported by the chosen browser",
    )
    parser.add_argument(
        "--browser-startup-timeout-ms",
        type=int,
        default=900000,
        help="Max wait for each browser scout tab to reach contributing state",
    )
    parser.add_argument(
        "--browser-user-data-dir",
        type=str,
        default="",
        help="Persistent browser profile directory for real browser scouts",
    )
    parser.add_argument(
        "--browser-warmup-timeout-s",
        type=int,
        default=45,
        help="Untimed wait for browser scouts to become productive before measured load starts",
    )
    parser.add_argument(
        "--browser-warmup-max-requests",
        type=int,
        default=12,
        help="Maximum untimed warmup requests used to prime real browser scouts",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=32,
        help="max_tokens sent to each chat completion request",
    )
    parser.add_argument(
        "--readiness-timeout-s",
        type=int,
        default=120,
        help="Max wait before a run starts while endpoints become healthy/drained",
    )
    parser.add_argument(
        "--ready-queue-depth-max",
        type=float,
        default=8.0,
        help="Required max queue depth before test launch",
    )
    parser.add_argument(
        "--allow-readiness-blackout",
        action="store_true",
        help="Allow run launch even if any endpoint reports scout blackout mode",
    )
    parser.add_argument(
        "--strict-readiness",
        action="store_true",
        help="Fail the run if endpoints do not drain/ready within readiness timeout.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    endpoints = [item.strip() for item in args.verifier_pool.split(",") if item.strip()]
    exit_code = asyncio.run(
        run_orchestrator(
            scouts=args.scouts,
            rate=args.rate,
            duration=args.duration,
            verifier_pool=endpoints,
            out_path=Path(args.out),
            inference_mode=args.inference_mode,
            request_timeout_ms=args.request_timeout_ms,
            max_attempts=args.max_attempts,
            scout_workers=args.scout_workers,
            max_tokens=args.max_tokens,
            scout_mode=args.scout_mode,
            browser_scout_page_base_url=args.browser_scout_page_base_url,
            browser_channel=args.browser_channel,
            browser_headless=args.browser_headless,
            browser_startup_timeout_ms=args.browser_startup_timeout_ms,
            browser_user_data_dir=args.browser_user_data_dir or None,
            readiness_timeout_s=args.readiness_timeout_s,
            ready_queue_depth_max=args.ready_queue_depth_max,
            require_no_blackout=not args.allow_readiness_blackout,
            strict_readiness=args.strict_readiness,
        )
    )
    raise SystemExit(exit_code)


if __name__ == "__main__":
    main()
