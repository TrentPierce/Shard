#!/usr/bin/env python3
"""
Release-candidate benchmark matrix runner with go/no-go output.

Runs repeated orchestrator scenarios and emits:
  - go-no-go-summary.json
  - go-no-go-report.md

Usage example:
  python benchmarks/distributed/run_release_matrix.py \
    --one-node-pool http://127.0.0.1:9191 \
    --two-node-pool http://127.0.0.1:9191,http://35.175.242.222:9091 \
    --runs-per-scenario 3
"""

from __future__ import annotations

import argparse
import asyncio
import json
import statistics
import subprocess
import sys
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import httpx

if __package__ is None or __package__ == "":
    sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from benchmarks.distributed.orchestrator import run_orchestrator, wait_for_pool_readiness


@dataclass(frozen=True)
class Scenario:
    name: str
    use_scout_workers: bool
    pool_kind: str  # one_node | two_node


def scenario_inference_mode(scenario: Scenario, default_mode: str) -> str:
    # "No scouts" scenarios are intended to measure verifier-only behavior.
    return default_mode if scenario.use_scout_workers else "standard"


SCENARIOS: list[Scenario] = [
    Scenario(name="one-node-no-scouts", use_scout_workers=False, pool_kind="one_node"),
    Scenario(name="two-node-no-scouts", use_scout_workers=False, pool_kind="two_node"),
    Scenario(name="one-node-with-scouts", use_scout_workers=True, pool_kind="one_node"),
    Scenario(name="two-node-with-scouts", use_scout_workers=True, pool_kind="two_node"),
]


def parse_pool(raw: str) -> list[str]:
    endpoints = [item.strip() for item in raw.split(",") if item.strip()]
    if not endpoints:
        raise ValueError("verifier pool cannot be empty")
    return endpoints


def median_metric(rows: list[dict[str, Any]], key: str) -> float:
    values = [float(row.get(key, 0.0) or 0.0) for row in rows]
    if not values:
        return 0.0
    return float(statistics.median(values))


def mean_metric(rows: list[dict[str, Any]], key: str) -> float:
    values = [float(row.get(key, 0.0) or 0.0) for row in rows]
    if not values:
        return 0.0
    return float(statistics.fmean(values))


def nested_count(row: dict[str, Any], section: str, key: str) -> float:
    payload = row.get(section, {})
    if not isinstance(payload, dict):
        return 0.0
    return float(payload.get(key, 0.0) or 0.0)


def median_rate_pct(
    rows: list[dict[str, Any]],
    numerator_fn,
) -> float:
    values: list[float] = []
    for row in rows:
        total = float(row.get("total_requests", 0.0) or 0.0)
        if total <= 0:
            continue
        values.append((float(numerator_fn(row)) / total) * 100.0)
    if not values:
        return 0.0
    return float(statistics.median(values))


def git_short_head() -> str:
    try:
        return (
            subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True)
            .strip()
        )
    except Exception:
        return "unknown"


async def probe_endpoint_latency_ms(
    endpoint: str,
    max_tokens: int,
    timeout_s: float = 120.0,
) -> float:
    payload = {
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": "hello from release calibration probe"}],
        "max_tokens": max(1, max_tokens),
        "stream": False,
    }
    timeout = httpx.Timeout(timeout_s, connect=min(5.0, timeout_s))
    started = datetime.now(UTC).timestamp()
    async with httpx.AsyncClient(timeout=timeout) as client:
        resp = await client.post(
            f"{endpoint.rstrip('/')}/v1/chat/completions",
            headers={
                "x-shard-inference-mode": "standard",
                "x-shard-mesh-forward": "false",
            },
            json=payload,
        )
        resp.raise_for_status()
        await resp.aread()
    finished = datetime.now(UTC).timestamp()
    return max(1.0, (finished - started) * 1000.0)


async def calibrate_scenario_load(
    pool: list[str],
    configured_rate: float,
    request_timeout_ms: int,
    max_tokens: int,
    utilization: float,
    timeout_multiplier: float,
) -> tuple[float, int, dict[str, float]]:
    latencies: dict[str, float] = {}
    for endpoint in pool:
        try:
            latencies[endpoint] = await probe_endpoint_latency_ms(endpoint, max_tokens=max_tokens)
        except Exception:
            continue

    if not latencies:
        return configured_rate, request_timeout_ms, {}

    service_capacity_rps = sum(1000.0 / max(latency_ms, 1.0) for latency_ms in latencies.values())
    calibrated_rate = min(configured_rate, max(0.05, service_capacity_rps * utilization))
    calibrated_timeout_ms = max(
        request_timeout_ms,
        int(max(latencies.values()) * timeout_multiplier) + 2_000,
    )
    return calibrated_rate, calibrated_timeout_ms, latencies


async def configure_scout_ingress(
    verifier_pool: list[str],
    enabled: bool,
    timeout_s: float = 10.0,
) -> None:
    timeout = httpx.Timeout(timeout_s, connect=min(3.0, timeout_s))
    async with httpx.AsyncClient(timeout=timeout) as client:
        for endpoint in verifier_pool:
            resp = await client.post(
                f"{endpoint.rstrip('/')}/v1/system/scout-ingress",
                json={"enabled": enabled},
            )
            resp.raise_for_status()
            payload = resp.json() or {}
            if bool(payload.get("scout_ingress_enabled", enabled)) != enabled:
                raise RuntimeError(
                    f"{endpoint}: failed to set scout ingress to {enabled}"
                )


def evaluate_release_gates(aggregates: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    two_with = aggregates.get("two-node-with-scouts", {})
    two_no = aggregates.get("two-node-no-scouts", {})
    p95_with = float(two_with.get("p95_latency_ms_median", 0.0))
    err_with = float(two_with.get("error_rate_pct_median", 100.0))
    tps_with = float(two_with.get("throughput_tps_median", 0.0))
    timeout_with = float(two_with.get("timeout_rate_pct_median", 100.0))
    http429_with = float(two_with.get("http_429_rate_pct_median", 100.0))
    p95_no = float(two_no.get("p95_latency_ms_median", 0.0))
    tps_no = float(two_no.get("throughput_tps_median", 0.0))

    gates: list[dict[str, Any]] = [
        {
            "name": "two_node_with_scouts_p95_ms",
            "description": "2-node with scouts p95 latency must be <= 4500ms",
            "actual": round(p95_with, 3),
            "expected_op": "<=",
            "expected": 4500.0,
            "pass": p95_with <= 4500.0,
        },
        {
            "name": "two_node_with_scouts_error_pct",
            "description": "2-node with scouts error rate must be <= 3%",
            "actual": round(err_with, 4),
            "expected_op": "<=",
            "expected": 3.0,
            "pass": err_with <= 3.0,
        },
        {
            "name": "two_node_with_scouts_tps",
            "description": "2-node with scouts throughput must be >= 3.75 TPS",
            "actual": round(tps_with, 4),
            "expected_op": ">=",
            "expected": 3.75,
            "pass": tps_with >= 3.75,
        },
        {
            "name": "two_node_with_scouts_timeout_rate_pct",
            "description": "2-node with scouts timeout rate must be <= 2%",
            "actual": round(timeout_with, 4),
            "expected_op": "<=",
            "expected": 2.0,
            "pass": timeout_with <= 2.0,
        },
        {
            "name": "two_node_with_scouts_http_429_rate_pct",
            "description": "2-node with scouts HTTP 429 rate must be <= 5%",
            "actual": round(http429_with, 4),
            "expected_op": "<=",
            "expected": 5.0,
            "pass": http429_with <= 5.0,
        },
    ]

    # Relative gates are only valid if two-node no-scouts has non-zero reference values.
    if tps_no > 0:
        gates.append(
            {
                "name": "two_node_with_scouts_tps_vs_no_scouts",
                "description": "2-node with scouts TPS must be >= 105% of 2-node no-scouts",
                "actual": round(tps_with, 4),
                "expected_op": ">=",
                "expected": round(tps_no * 1.05, 4),
                "pass": tps_with >= (tps_no * 1.05),
            }
        )
    else:
        gates.append(
            {
                "name": "two_node_with_scouts_tps_vs_no_scouts",
                "description": "2-node with scouts TPS must be >= 105% of 2-node no-scouts",
                "actual": round(tps_with, 4),
                "expected_op": "n/a",
                "expected": None,
                "pass": False,
            }
        )

    if p95_no > 0:
        gates.append(
            {
                "name": "two_node_with_scouts_p95_vs_no_scouts",
                "description": "2-node with scouts p95 must be <= 110% of 2-node no-scouts",
                "actual": round(p95_with, 3),
                "expected_op": "<=",
                "expected": round(p95_no * 1.10, 3),
                "pass": p95_with <= (p95_no * 1.10),
            }
        )
    else:
        gates.append(
            {
                "name": "two_node_with_scouts_p95_vs_no_scouts",
                "description": "2-node with scouts p95 must be <= 110% of 2-node no-scouts",
                "actual": round(p95_with, 3),
                "expected_op": "n/a",
                "expected": None,
                "pass": False,
            }
        )

    return gates


def markdown_report(summary: dict[str, Any]) -> str:
    lines: list[str] = []
    lines.append("# Release RC Go/No-Go Report")
    lines.append("")
    lines.append(f"- Generated (UTC): `{summary['generated_at_utc']}`")
    lines.append(f"- Commit: `{summary['git_commit']}`")
    lines.append(f"- Recommendation: **{summary['recommendation']}**")
    lines.append("")
    lines.append("## Scenario Medians")
    lines.append("")
    lines.append("| Scenario | p95 (ms) | TPS | Error (%) | Timeout (%) | HTTP 429 (%) | Runs |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|")
    for scenario in SCENARIOS:
        row = summary["aggregates"][scenario.name]
        lines.append(
            f"| {scenario.name} | {row['p95_latency_ms_median']:.3f} | "
            f"{row['throughput_tps_median']:.4f} | {row['error_rate_pct_median']:.4f} | "
            f"{row['timeout_rate_pct_median']:.4f} | {row['http_429_rate_pct_median']:.4f} | "
            f"{row['runs']} |"
        )
    lines.append("")
    lines.append("## Gate Evaluation")
    lines.append("")
    lines.append("| Gate | Actual | Target | Pass |")
    lines.append("|---|---:|---:|---|")
    for gate in summary["gates"]:
        target = (
            "n/a"
            if gate["expected"] is None
            else f"{gate['expected_op']} {gate['expected']}"
        )
        status = "PASS" if gate["pass"] else "FAIL"
        lines.append(f"| {gate['name']} | {gate['actual']} | {target} | {status} |")
    lines.append("")
    lines.append("## Notes")
    lines.append("")
    lines.append("- Recommendation is **GO** only if all gates pass.")
    lines.append("- Use this report with raw JSON run artifacts for release signoff.")
    lines.append("")
    return "\n".join(lines)


async def execute(args: argparse.Namespace) -> dict[str, Any]:
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    suffix = f"-{args.tag.strip()}" if args.tag and args.tag.strip() else ""
    run_dir = Path(args.out_dir) / f"release-rc-{stamp}{suffix}"
    run_dir.mkdir(parents=True, exist_ok=True)

    one_pool = parse_pool(args.one_node_pool)
    two_pool = parse_pool(args.two_node_pool)
    all_endpoints = list(dict.fromkeys(one_pool + two_pool))
    by_scenario: dict[str, list[dict[str, Any]]] = {scenario.name: [] for scenario in SCENARIOS}

    try:
        for scenario in SCENARIOS:
            pool = one_pool if scenario.pool_kind == "one_node" else two_pool
            workers = args.scout_workers if scenario.use_scout_workers else 0
            await configure_scout_ingress(pool, enabled=scenario.use_scout_workers)
            scenario_rate = args.rate
            scenario_timeout_ms = args.request_timeout_ms
            latency_probe_ms: dict[str, float] = {}
            if args.auto_calibrate_rate:
                scenario_rate, scenario_timeout_ms, latency_probe_ms = await calibrate_scenario_load(
                    pool=pool,
                    configured_rate=args.rate,
                    request_timeout_ms=args.request_timeout_ms,
                    max_tokens=args.max_tokens,
                    utilization=args.auto_calibrate_utilization,
                    timeout_multiplier=args.auto_timeout_multiplier,
                )
            for run_idx in range(1, args.runs_per_scenario + 1):
                out_path = run_dir / f"{scenario.name}-run{run_idx}.json"
                print(
                    f"Running {scenario.name} (run {run_idx}/{args.runs_per_scenario}) "
                    f"pool={','.join(pool)} scout_workers={workers} "
                    f"inference_mode={scenario_inference_mode(scenario, args.inference_mode)} "
                    f"rate={scenario_rate:.4f} timeout_ms={scenario_timeout_ms}"
                )
                exit_code = await run_orchestrator(
                    scouts=args.scouts,
                    rate=scenario_rate,
                    duration=args.duration,
                    verifier_pool=pool,
                    out_path=out_path,
                    inference_mode=scenario_inference_mode(scenario, args.inference_mode),
                    request_timeout_ms=scenario_timeout_ms,
                    max_attempts=args.max_attempts,
                    scout_workers=workers,
                    max_tokens=args.max_tokens,
                    scout_mode=args.scout_mode if scenario.use_scout_workers else "synthetic",
                    browser_scout_page_base_url=args.browser_scout_page_base_url,
                    browser_channel=args.browser_channel,
                    browser_headless=args.browser_headless,
                    browser_startup_timeout_ms=args.browser_startup_timeout_ms,
                    browser_user_data_dir=args.browser_user_data_dir or None,
                    browser_warmup_timeout_s=args.browser_warmup_timeout_s,
                    browser_warmup_max_requests=args.browser_warmup_max_requests,
                    readiness_timeout_s=args.readiness_timeout_s,
                    ready_queue_depth_max=args.ready_queue_depth_max,
                    require_no_blackout=not args.allow_readiness_blackout,
                    strict_readiness=args.strict_readiness,
                )
                try:
                    data = json.loads(out_path.read_text(encoding="utf-8"))
                except Exception as exc:
                    data = {
                        "error_rate_pct": 100.0,
                        "p95_latency_ms": 0.0,
                        "throughput_tps": 0.0,
                        "run_error": f"failed_to_read_output: {exc}",
                    }
                data["orchestrator_exit_code"] = exit_code
                data["scenario_inference_mode"] = scenario_inference_mode(
                    scenario, args.inference_mode
                )
                data["scenario_rate"] = scenario_rate
                data["scenario_request_timeout_ms"] = scenario_timeout_ms
                data["scenario_latency_probe_ms"] = latency_probe_ms
                data["output_file"] = str(out_path.as_posix())
                by_scenario[scenario.name].append(data)
                if args.flush_timeout_s > 0:
                    print(
                        "Flushing verifier queues before next run "
                        f"(timeout={args.flush_timeout_s}s queue_max={args.flush_queue_depth_max})"
                    )
                    flush_timeout = httpx.Timeout(
                        connect=2.0,
                        read=min(10.0, max(2.0, args.flush_timeout_s)),
                        write=min(10.0, max(2.0, args.flush_timeout_s)),
                        pool=min(10.0, max(2.0, args.flush_timeout_s)),
                    )
                    async with httpx.AsyncClient(timeout=flush_timeout) as flush_client:
                        await wait_for_pool_readiness(
                            client=flush_client,
                            verifier_pool=pool,
                            timeout_s=args.flush_timeout_s,
                            queue_depth_max=args.flush_queue_depth_max,
                            require_no_blackout=not args.allow_readiness_blackout,
                            strict=args.strict_flush_readiness,
                        )
    finally:
        await configure_scout_ingress(all_endpoints, enabled=True)

    aggregates: dict[str, dict[str, Any]] = {}
    for scenario in SCENARIOS:
        rows = by_scenario.get(scenario.name, [])
        aggregates[scenario.name] = {
            "runs": len(rows),
            "p95_latency_ms_median": round(median_metric(rows, "p95_latency_ms"), 3),
            "throughput_tps_median": round(median_metric(rows, "throughput_tps"), 4),
            "error_rate_pct_median": round(median_metric(rows, "error_rate_pct"), 4),
            "timeout_rate_pct_median": round(
                median_rate_pct(
                    rows,
                    lambda row: nested_count(row, "error_breakdown", "ReadTimeout")
                    + nested_count(row, "error_breakdown", "ConnectTimeout"),
                ),
                4,
            ),
            "http_429_rate_pct_median": round(
                median_rate_pct(rows, lambda row: nested_count(row, "status_counts", "429")),
                4,
            ),
            "http_503_rate_pct_median": round(
                median_rate_pct(rows, lambda row: nested_count(row, "status_counts", "503")),
                4,
            ),
            "p95_latency_ms_mean": round(mean_metric(rows, "p95_latency_ms"), 3),
            "throughput_tps_mean": round(mean_metric(rows, "throughput_tps"), 4),
            "error_rate_pct_mean": round(mean_metric(rows, "error_rate_pct"), 4),
            "all_orchestrator_runs_passed": all(
                int(row.get("orchestrator_exit_code", 1)) == 0 for row in rows
            ),
        }

    gates = evaluate_release_gates(aggregates)
    all_pass = all(bool(gate.get("pass")) for gate in gates)
    recommendation = "GO" if all_pass else "NO_GO"

    summary: dict[str, Any] = {
        "generated_at_utc": datetime.now(UTC).isoformat(),
        "git_commit": git_short_head(),
        "recommendation": recommendation,
        "config": {
            "scouts": args.scouts,
            "rate": args.rate,
            "duration": args.duration,
            "runs_per_scenario": args.runs_per_scenario,
            "scout_workers_with_scouts": args.scout_workers,
            "inference_mode": args.inference_mode,
            "request_timeout_ms": args.request_timeout_ms,
            "auto_calibrate_rate": args.auto_calibrate_rate,
            "auto_calibrate_utilization": args.auto_calibrate_utilization,
            "auto_timeout_multiplier": args.auto_timeout_multiplier,
            "max_attempts": args.max_attempts,
            "max_tokens": args.max_tokens,
            "scout_mode": args.scout_mode,
            "browser_scout_page_base_url": args.browser_scout_page_base_url,
            "browser_channel": args.browser_channel,
            "browser_headless": args.browser_headless,
            "browser_startup_timeout_ms": args.browser_startup_timeout_ms,
            "browser_user_data_dir": args.browser_user_data_dir,
            "browser_warmup_timeout_s": args.browser_warmup_timeout_s,
            "browser_warmup_max_requests": args.browser_warmup_max_requests,
            "readiness_timeout_s": args.readiness_timeout_s,
            "ready_queue_depth_max": args.ready_queue_depth_max,
            "allow_readiness_blackout": args.allow_readiness_blackout,
            "strict_readiness": args.strict_readiness,
            "strict_flush_readiness": args.strict_flush_readiness,
            "flush_timeout_s": args.flush_timeout_s,
            "flush_queue_depth_max": args.flush_queue_depth_max,
            "one_node_pool": one_pool,
            "two_node_pool": two_pool,
        },
        "aggregates": aggregates,
        "gates": gates,
        "raw_runs": by_scenario,
    }

    summary_path = run_dir / "go-no-go-summary.json"
    summary_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    report_path = run_dir / "go-no-go-report.md"
    report_path.write_text(markdown_report(summary), encoding="utf-8")

    print(f"Go/No-Go summary: {summary_path}")
    print(f"Go/No-Go report: {report_path}")
    print(f"Recommendation: {recommendation}")
    return summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run repeated RC matrix and emit go/no-go summary"
    )
    parser.add_argument(
        "--one-node-pool",
        required=True,
        help="Comma-separated verifier pool for one-node scenarios",
    )
    parser.add_argument(
        "--two-node-pool",
        required=True,
        help="Comma-separated verifier pool for two-node scenarios",
    )
    parser.add_argument("--runs-per-scenario", type=int, default=3)
    parser.add_argument("--scouts", type=int, default=16)
    parser.add_argument("--rate", type=float, default=2.0)
    parser.add_argument("--duration", type=int, default=10)
    parser.add_argument("--scout-workers", type=int, default=2)
    parser.add_argument(
        "--scout-mode",
        type=str,
        default="synthetic",
        choices=["synthetic", "browser"],
    )
    parser.add_argument(
        "--browser-scout-page-base-url",
        type=str,
        default="http://127.0.0.1:3000/benchmark/scout",
    )
    parser.add_argument(
        "--browser-channel",
        type=str,
        default="msedge" if sys.platform.startswith("win") else "chrome",
    )
    parser.add_argument("--browser-headless", action="store_true")
    parser.add_argument("--browser-startup-timeout-ms", type=int, default=900000)
    parser.add_argument("--browser-user-data-dir", type=str, default="")
    parser.add_argument("--browser-warmup-timeout-s", type=int, default=45)
    parser.add_argument("--browser-warmup-max-requests", type=int, default=12)
    parser.add_argument(
        "--inference-mode",
        type=str,
        default="distributed",
        choices=["standard", "distributed", "speculative"],
    )
    parser.add_argument("--request-timeout-ms", type=int, default=30000)
    parser.add_argument("--auto-calibrate-rate", action="store_true")
    parser.add_argument("--auto-calibrate-utilization", type=float, default=0.85)
    parser.add_argument("--auto-timeout-multiplier", type=float, default=4.0)
    parser.add_argument("--max-attempts", type=int, default=1)
    parser.add_argument("--max-tokens", type=int, default=8)
    parser.add_argument("--readiness-timeout-s", type=int, default=30)
    parser.add_argument("--ready-queue-depth-max", type=float, default=3.0)
    parser.add_argument("--allow-readiness-blackout", action="store_true")
    parser.add_argument(
        "--allow-dirty-readiness",
        dest="strict_readiness",
        action="store_false",
        help="Allow scenarios to run even if verifier readiness never clears.",
    )
    parser.set_defaults(strict_readiness=True)
    parser.add_argument(
        "--flush-timeout-s",
        type=int,
        default=20,
        help="Wait this long for queue/blackout drain between runs (0 disables).",
    )
    parser.add_argument(
        "--flush-queue-depth-max",
        type=float,
        default=3.0,
        help="Required queue depth before the next scenario starts.",
    )
    parser.add_argument(
        "--allow-dirty-flush",
        dest="strict_flush_readiness",
        action="store_false",
        help="Allow the matrix to continue even when inter-run queue drain fails.",
    )
    parser.set_defaults(strict_flush_readiness=True)
    parser.add_argument("--out-dir", type=str, default="reports/release-rc")
    parser.add_argument("--tag", type=str, default="")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    summary = asyncio.run(execute(args))
    if summary.get("recommendation") != "GO":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
