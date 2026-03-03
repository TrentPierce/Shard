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
import json
import time
from dataclasses import dataclass
from pathlib import Path

import httpx


@dataclass
class RequestRecord:
    ok: bool
    latency_ms: float


class RoundRobinPool:
    def __init__(self, endpoints: list[str]) -> None:
        self._endpoints = [ep.rstrip("/") for ep in endpoints if ep.strip()]
        self._idx = 0
        self._lock = asyncio.Lock()

    async def next(self) -> str:
        async with self._lock:
            endpoint = self._endpoints[self._idx % len(self._endpoints)]
            self._idx += 1
            return endpoint


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
) -> int:
    if not verifier_pool:
        raise ValueError("verifier_pool cannot be empty")

    pool = RoundRobinPool(verifier_pool)
    semaphore = asyncio.Semaphore(max(1, scouts))
    records: list[RequestRecord] = []
    records_lock = asyncio.Lock()
    in_flight = 0
    in_flight_lock = asyncio.Lock()
    timeseries: list[dict] = []

    start = time.monotonic()
    end = start + duration

    limits = httpx.Limits(max_keepalive_connections=max(50, scouts), max_connections=max(100, scouts * 2))
    timeout = httpx.Timeout(connect=5.0, read=30.0, write=30.0, pool=30.0)

    async with httpx.AsyncClient(limits=limits, timeout=timeout) as client:
        baseline = await fetch_summary(client, verifier_pool[0])

        async def fire_one(seq: int) -> None:
            nonlocal in_flight
            async with semaphore:
                async with in_flight_lock:
                    in_flight += 1
                t0 = time.monotonic()
                ok = False
                try:
                    endpoint = await pool.next()
                    resp = await client.post(
                        f"{endpoint}/v1/chat/completions",
                        json={
                            "model": "shard-hybrid",
                            "messages": [{"role": "user", "content": f"hello from scout {seq}"}],
                            "max_tokens": 24,
                        },
                    )
                    ok = resp.status_code < 500
                except Exception:
                    ok = False
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

        await asyncio.gather(launcher(), progress_loop())
        await asyncio.gather(*launch_tasks, return_exceptions=True)

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
    acceptance_rate_pct = (accepted_delta / total_delta * 100.0) if total_delta > 0 else float(after.get("speculative_acceptance_rate", 0.0)) * 100.0

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
        )
    )
    raise SystemExit(exit_code)


if __name__ == "__main__":
    main()
