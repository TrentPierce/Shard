#!/usr/bin/env python3
"""
Shard Benchmark Harness - Entry Point
Usage: python benchmarks/run_all.py --scouts 100 --duration 300 --base-url http://localhost:9091
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
import time
from pathlib import Path

import httpx

if __package__ is None or __package__ == "":
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from benchmarks.gates import evaluate_gates
from benchmarks.metrics_collector import MetricsCollector
from benchmarks.report_generator import ReportGenerator
from benchmarks.scout_simulator import ScoutSimulator


async def measure_baseline(url: str, samples: int) -> list[float]:
    latencies: list[float] = []
    async with httpx.AsyncClient(timeout=30.0) as client:
        for _ in range(samples):
            start = time.monotonic()
            resp = await client.post(
                f"{url.rstrip('/')}/v1/chat/completions",
                json={
                    "model": "shard-hybrid",
                    "messages": [{"role": "user", "content": "Say hello in one sentence."}],
                },
            )
            resp.raise_for_status()
            latencies.append(time.monotonic() - start)
    return latencies


async def main() -> None:
    parser = argparse.ArgumentParser(description="Shard Benchmark Harness")
    parser.add_argument(
        "--scouts",
        type=int,
        default=100,
        help="Number of concurrent simulated Scout nodes",
    )
    parser.add_argument("--duration", type=int, default=300, help="Test duration in seconds")
    parser.add_argument("--base-url", default="http://localhost:9091", help="Daemon base URL")
    parser.add_argument(
        "--baseline-url",
        default=None,
        help="Baseline single-node URL for latency comparison",
    )
    parser.add_argument("--out-dir", default="benchmarks/results", help="Output directory")
    parser.add_argument(
        "--tensor-parallel-degree",
        type=int,
        default=1,
        help="Tensor parallel degree marker for benchmark comparison runs",
    )
    args = parser.parse_args()

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    print(f"Starting benchmark: {args.scouts} scouts, {args.duration}s duration")

    collector = MetricsCollector(base_url=args.base_url)
    await collector.capture_metrics_baseline()

    simulator = ScoutSimulator(
        base_url=args.base_url,
        scout_count=args.scouts,
        duration_seconds=args.duration,
        collector=collector,
    )

    baseline_latencies: list[float] = []
    if args.baseline_url:
        print("Measuring baseline latency...")
        baseline_latencies = await measure_baseline(args.baseline_url, samples=50)

    print("Running Scout simulation...")
    await simulator.run()

    await collector.capture_metrics_after()
    results = collector.summarize(
        baseline_latencies=baseline_latencies,
        scout_count=args.scouts,
        duration_seconds=args.duration,
        base_url=args.base_url,
    )
    results["tensor_parallel_degree"] = max(1, args.tensor_parallel_degree)

    results_path = out_dir / "latest.json"
    with results_path.open("w", encoding="utf-8") as f:
        json.dump(results, f, indent=2, sort_keys=True)
    print(f"Results written to {results_path}")

    report = ReportGenerator(results).render()
    report_path = out_dir / "report.md"
    report_path.write_text(report, encoding="utf-8")
    print(f"Report written to {report_path}")

    passed, failures = evaluate_gates(results)
    if passed:
        print("\nALL GATES PASSED - Phase 1 benchmark complete")
    else:
        print("\nGATE FAILURES:")
        for failure in failures:
            print(f"  - {failure}")
        raise SystemExit(1)


if __name__ == "__main__":
    asyncio.run(main())
