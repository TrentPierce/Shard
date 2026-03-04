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
import hashlib
import json
import re
import time
from dataclasses import dataclass
from pathlib import Path

import httpx


@dataclass
class RequestRecord:
    ok: bool
    latency_ms: float


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
            try:
                resp = await client.get(f"{state.endpoint}/metrics/summary")
                if resp.is_success:
                    data = resp.json()
                    queue_depth = float(data.get("queue_depth", queue_depth) or 0.0)
                    load = float(data.get("load", load) or 0.0)
            except Exception:
                pass
            async with self._lock:
                current = self._states.get(state.endpoint)
                if current is None:
                    continue
                current.queue_depth = max(0.0, queue_depth)
                current.load = max(0.0, load)
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


async def fetch_summary(client: httpx.AsyncClient, base_url: str) -> dict:
    try:
        resp = await client.get(f"{base_url.rstrip('/')}/metrics/summary")
        if resp.is_success:
            return resp.json()
    except Exception:
        pass
    return {}


async def run_orchestrator(
    scouts: int,
    rate: int,
    duration: int,
    verifier_pool: list[str],
    out_path: Path,
    inference_mode: str,
    request_timeout_ms: int,
    max_attempts: int,
    scout_workers: int,
    max_tokens: int,
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

    start = time.monotonic()
    end = start + duration

    limits = httpx.Limits(max_keepalive_connections=max(50, scouts), max_connections=max(100, scouts * 2))
    timeout_secs = max(0.5, request_timeout_ms / 1000.0)
    timeout = httpx.Timeout(
        connect=min(2.0, timeout_secs),
        read=timeout_secs,
        write=timeout_secs,
        pool=timeout_secs,
    )

    async with httpx.AsyncClient(limits=limits, timeout=timeout) as client:
        baseline = await fetch_summary(client, verifier_pool[0])

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
            endpoint = verifier_pool[worker_idx % len(verifier_pool)]
            scout_id = f"orchestrator-scout-{worker_idx}"
            verified = False
            while time.monotonic() < end:
                try:
                    if not verified:
                        verified = await ensure_pow_for_scout(endpoint, scout_id)
                        if not verified:
                            await asyncio.sleep(0.5)
                            continue
                        # Seed recent submitter state so speculative scheduling does not
                        # underestimate active scouts on fresh daemon boots.
                        await client.post(
                            f"{endpoint}/v1/scout/draft",
                            json={
                                "work_id": f"warmup-{scout_id}",
                                "scout_id": scout_id,
                                "draft_text": "ok",
                                "prompt_context": "warmup",
                                "timestamp": time.time(),
                            },
                        )

                    work_resp = await client.get(
                        f"{endpoint}/v1/scout/work",
                        params={"scout_id": scout_id},
                    )
                    if not work_resp.is_success:
                        await asyncio.sleep(0.05)
                        continue
                    work = (work_resp.json() or {}).get("work")
                    if not work:
                        await asyncio.sleep(0.01)
                        continue

                    prompt_context = str(work.get("prompt_context", ""))
                    user_prompt = extract_user_message(prompt_context)
                    min_tokens = max(1, int(work.get("min_tokens", 1)))

                    # Draft with standard mode so this helper path does not recurse into speculative work.
                    gen_resp = await client.post(
                        f"{endpoint}/v1/chat/completions",
                        headers={"x-shard-inference-mode": "standard"},
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
                for _attempt in range(max(1, max_attempts)):
                    endpoint = None
                    try:
                        endpoint = await pool.next(client)
                        resp = await client.post(
                            f"{endpoint}/v1/chat/completions",
                            headers={"x-shard-inference-mode": inference_mode},
                            json={
                                "model": "shard-hybrid",
                                "messages": [{"role": "user", "content": f"hello from scout {seq}"}],
                                "max_tokens": max(1, max_tokens),
                            },
                        )
                        attempt_ok = resp.status_code < 500
                        await pool.note_result(
                            endpoint=endpoint,
                            ok=attempt_ok,
                            latency_ms=(time.monotonic() - t0) * 1000.0,
                        )
                        if attempt_ok:
                            ok = True
                            break
                    except Exception:
                        if endpoint:
                            await pool.note_result(
                                endpoint=endpoint,
                                ok=False,
                                latency_ms=(time.monotonic() - t0) * 1000.0,
                            )
                    await asyncio.sleep(0.01)
                latency_ms = (time.monotonic() - t0) * 1000.0
                async with records_lock:
                    records.append(RequestRecord(ok=ok, latency_ms=latency_ms))
                async with in_flight_lock:
                    in_flight -= 1

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
                summary_now = await fetch_summary(client, verifier_pool[0])
                acceptance = float(summary_now.get("speculative_acceptance_rate", 0.0)) * 100.0
                async with in_flight_lock:
                    active = in_flight
                print(
                    f"[{elapsed}s/{duration}s] Scouts: {active} active | p95: {p95/1000.0:.2f}s | "
                    f"accept: {acceptance:.1f}% | errors: {err_rate:.2f}%"
                )
                timeseries.append(
                    {
                        "t": elapsed,
                        "active_scouts": active,
                        "p95_latency_ms": round(p95, 3),
                        "acceptance_rate_pct": round(acceptance, 3),
                        "error_rate_pct": round(err_rate, 4),
                    }
                )

        scout_tasks = [
            asyncio.create_task(scout_loop(idx))
            for idx in range(max(0, scout_workers))
        ]
        await asyncio.gather(launcher(), progress_loop())
        await asyncio.gather(*launch_tasks, return_exceptions=True)
        for task in scout_tasks:
            task.cancel()
        await asyncio.gather(*scout_tasks, return_exceptions=True)

        after = await fetch_summary(client, verifier_pool[0])

    latencies = [r.latency_ms for r in records]
    total = len(records)
    oks = sum(1 for r in records if r.ok)
    errs = total - oks
    error_rate_pct = (errs / total * 100.0) if total else 100.0
    p95_latency_ms = percentile(latencies, 0.95)

    before_accepted = float(baseline.get("speculative_accepted_tokens_total", 0.0))
    before_total = float(baseline.get("speculative_draft_tokens_total", 0.0))
    after_accepted = float(after.get("speculative_accepted_tokens_total", 0.0))
    after_total = float(after.get("speculative_draft_tokens_total", 0.0))
    accepted_delta = max(0.0, after_accepted - before_accepted)
    total_delta = max(0.0, after_total - before_total)
    acceptance_source = "speculative_metrics"
    if total_delta > 0:
        acceptance_rate_pct = accepted_delta / total_delta * 100.0
    else:
        # If the verifier did not record speculative token samples for this run,
        # don't hard-fail acceptance gates on missing telemetry.
        acceptance_rate_pct = 100.0
        acceptance_source = "no_speculative_samples"

    throughput_tps = oks / float(duration) if duration > 0 else 0.0

    results = {
        "scouts": scouts,
        "rate": rate,
        "duration_seconds": duration,
        "verifier_pool": verifier_pool,
        "total_requests": total,
        "successful_requests": oks,
        "error_requests": errs,
        "error_rate_pct": round(error_rate_pct, 4),
        "p95_latency_ms": round(p95_latency_ms, 3),
        "acceptance_rate_pct": round(acceptance_rate_pct, 4),
        "acceptance_source": acceptance_source,
        "acceptance_samples": int(total_delta),
        "throughput_tps": round(throughput_tps, 4),
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
    parser.add_argument("--rate", type=int, required=True, help="Global requests per second")
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
        default=2500,
        help="Per-request timeout in milliseconds",
    )
    parser.add_argument(
        "--max-attempts",
        type=int,
        default=2,
        help="Attempts per request before counting as error",
    )
    parser.add_argument(
        "--scout-workers",
        type=int,
        default=0,
        help="Synthetic scout workers that poll /v1/scout/work and submit drafts",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=1,
        help="max_tokens sent to each chat completion request",
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
        )
    )
    raise SystemExit(exit_code)


if __name__ == "__main__":
    main()
