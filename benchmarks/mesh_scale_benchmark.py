#!/usr/bin/env python3
"""
Real mesh-scale benchmark runner for Shard.

This tool intentionally avoids synthetic estimates and random acceptance simulation.
All reported values are measured from live API responses and /metrics/summary counters.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import csv
import hashlib
import json
import math
import os
import random
import statistics
import subprocess
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import requests


DEFAULT_PROMPT = (
    "You are benchmarking distributed inference. Respond with three short bullets on "
    "why speculative decoding can reduce verifier work."
)


@dataclass
class Scenario:
    name: str
    node_count: int
    base_url: str


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def safe_get_json(url: str, timeout_s: float = 10.0) -> dict[str, Any]:
    response = requests.get(url, timeout=timeout_s)
    response.raise_for_status()
    return response.json()


def parse_scenarios(path: Path) -> list[Scenario]:
    data = json.loads(path.read_text(encoding="utf-8"))
    scenarios: list[Scenario] = []
    for item in data.get("scenarios", []):
        scenarios.append(
            Scenario(
                name=str(item["name"]),
                node_count=int(item["node_count"]),
                base_url=str(item["base_url"]).rstrip("/"),
            )
        )
    if not scenarios:
        raise ValueError("No scenarios found in scenarios JSON")
    return scenarios


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    sorted_values = sorted(values)
    idx = int(round((len(sorted_values) - 1) * p))
    return sorted_values[idx]


def confidence_interval_95(values: list[float]) -> tuple[float, float]:
    if not values:
        return 0.0, 0.0
    mean = statistics.fmean(values)
    if len(values) < 2:
        return mean, mean
    sd = statistics.stdev(values)
    margin = 1.96 * (sd / math.sqrt(len(values)))
    return mean - margin, mean + margin


def count_output_tokens(payload: dict[str, Any]) -> int:
    choices = payload.get("choices")
    if isinstance(choices, list) and choices:
        first = choices[0]
        if isinstance(first, dict):
            msg = first.get("message")
            if isinstance(msg, dict):
                content = msg.get("content", "")
                if isinstance(content, str):
                    return len(content.split())
    text = json.dumps(payload, ensure_ascii=True)
    return len(text.split())


def run_one_request(
    base_url: str,
    inference_mode: str,
    prompt: str,
    timeout_s: float,
) -> dict[str, Any]:
    request_id = str(uuid.uuid4())
    body = {
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": prompt}],
        "stream": False,
    }
    headers = {
        "Content-Type": "application/json",
        "X-Shard-Inference-Mode": inference_mode,
    }
    started = time.perf_counter()
    success = False
    status_code = 0
    output_tokens = 0
    error = ""
    try:
        response = requests.post(
            f"{base_url}/v1/chat/completions",
            json=body,
            headers=headers,
            timeout=timeout_s,
        )
        status_code = response.status_code
        response.raise_for_status()
        payload = response.json()
        output_tokens = count_output_tokens(payload)
        success = True
    except Exception as exc:  # noqa: BLE001
        error = str(exc)
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    return {
        "request_id": request_id,
        "latency_ms": elapsed_ms,
        "success": success,
        "status_code": status_code,
        "output_tokens": output_tokens,
        "error": error,
    }


def benchmark_scenario(
    scenario: Scenario,
    run_index: int,
    requests_per_run: int,
    concurrency: int,
    warmup_requests: int,
    inference_mode: str,
    request_timeout_s: float,
    prompt: str,
) -> dict[str, Any]:
    metrics_before = safe_get_json(f"{scenario.base_url}/metrics/summary")
    health_before = safe_get_json(f"{scenario.base_url}/health")

    for _ in range(max(0, warmup_requests)):
        run_one_request(
            scenario.base_url, inference_mode, prompt, timeout_s=request_timeout_s
        )

    started = time.perf_counter()
    events: list[dict[str, Any]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [
            executor.submit(
                run_one_request,
                scenario.base_url,
                inference_mode,
                prompt,
                request_timeout_s,
            )
            for _ in range(requests_per_run)
        ]
        for future in concurrent.futures.as_completed(futures):
            events.append(future.result())
    elapsed_s = max(0.001, time.perf_counter() - started)

    metrics_after = safe_get_json(f"{scenario.base_url}/metrics/summary")
    health_after = safe_get_json(f"{scenario.base_url}/health")

    successes = [e for e in events if e["success"]]
    failures = [e for e in events if not e["success"]]
    latencies = [float(e["latency_ms"]) for e in successes]
    output_tokens = sum(int(e["output_tokens"]) for e in successes)

    accepted_before = int(metrics_before.get("speculative_accepted_tokens_total", 0))
    accepted_after = int(metrics_after.get("speculative_accepted_tokens_total", 0))
    rejected_before = int(metrics_before.get("speculative_rejected_tokens_total", 0))
    rejected_after = int(metrics_after.get("speculative_rejected_tokens_total", 0))
    drafts_before = int(metrics_before.get("speculative_draft_tokens_total", 0))
    drafts_after = int(metrics_after.get("speculative_draft_tokens_total", 0))

    delta_accepted = max(0, accepted_after - accepted_before)
    delta_rejected = max(0, rejected_after - rejected_before)
    delta_drafts = max(0, drafts_after - drafts_before)

    measured_acceptance = (
        float(delta_accepted) / float(delta_drafts) if delta_drafts > 0 else None
    )
    measured_reject = (
        float(delta_rejected) / float(delta_drafts) if delta_drafts > 0 else None
    )

    return {
        "scenario": scenario.name,
        "node_count": scenario.node_count,
        "base_url": scenario.base_url,
        "run_index": run_index,
        "started_at_utc": now_iso(),
        "requests_total": requests_per_run,
        "concurrency": concurrency,
        "inference_mode": inference_mode,
        "elapsed_seconds": elapsed_s,
        "success_count": len(successes),
        "failure_count": len(failures),
        "success_rate": (len(successes) / requests_per_run) if requests_per_run else 0.0,
        "throughput_rps": len(successes) / elapsed_s,
        "output_tokens_total": output_tokens,
        "output_tokens_per_second": output_tokens / elapsed_s if output_tokens else 0.0,
        "latency_avg_ms": statistics.fmean(latencies) if latencies else 0.0,
        "latency_p50_ms": percentile(latencies, 0.50),
        "latency_p95_ms": percentile(latencies, 0.95),
        "latency_p99_ms": percentile(latencies, 0.99),
        "metrics_delta": {
            "speculative_draft_tokens_total": delta_drafts,
            "speculative_accepted_tokens_total": delta_accepted,
            "speculative_rejected_tokens_total": delta_rejected,
            "measured_acceptance_rate": measured_acceptance,
            "measured_reject_rate": measured_reject,
            "measured_speedup_ratio": metrics_after.get("speculative_speedup_ratio"),
            "verification_fallback_total_delta": max(
                0,
                int(metrics_after.get("verification_fallback_total", 0))
                - int(metrics_before.get("verification_fallback_total", 0)),
            ),
        },
        "health_before": health_before,
        "health_after": health_after,
        "metrics_before": metrics_before,
        "metrics_after": metrics_after,
        "request_events": events,
    }


def write_csv(path: Path, run_results: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            [
                "scenario",
                "node_count",
                "run_index",
                "request_id",
                "success",
                "status_code",
                "latency_ms",
                "output_tokens",
                "error",
            ]
        )
        for run in run_results:
            for event in run["request_events"]:
                writer.writerow(
                    [
                        run["scenario"],
                        run["node_count"],
                        run["run_index"],
                        event["request_id"],
                        event["success"],
                        event["status_code"],
                        f"{event['latency_ms']:.3f}",
                        event["output_tokens"],
                        event["error"],
                    ]
                )


def summarize(run_results: list[dict[str, Any]]) -> dict[str, Any]:
    by_scenario: dict[str, list[dict[str, Any]]] = {}
    for run in run_results:
        by_scenario.setdefault(run["scenario"], []).append(run)

    scenarios_summary: list[dict[str, Any]] = []
    for scenario, runs in by_scenario.items():
        tps = [float(r["throughput_rps"]) for r in runs]
        tokps = [float(r["output_tokens_per_second"]) for r in runs]
        p95 = [float(r["latency_p95_ms"]) for r in runs]
        success_rates = [float(r["success_rate"]) for r in runs]
        acceptance_rates = [
            r["metrics_delta"]["measured_acceptance_rate"]
            for r in runs
            if r["metrics_delta"]["measured_acceptance_rate"] is not None
        ]
        reject_rates = [
            r["metrics_delta"]["measured_reject_rate"]
            for r in runs
            if r["metrics_delta"]["measured_reject_rate"] is not None
        ]
        node_count = runs[0]["node_count"]
        ci_low, ci_high = confidence_interval_95(tps)
        tok_ci_low, tok_ci_high = confidence_interval_95(tokps)
        scenarios_summary.append(
            {
                "scenario": scenario,
                "node_count": node_count,
                "runs": len(runs),
                "throughput_rps_mean": statistics.fmean(tps),
                "throughput_rps_ci95_low": ci_low,
                "throughput_rps_ci95_high": ci_high,
                "output_tokens_per_second_mean": statistics.fmean(tokps),
                "output_tokens_per_second_ci95_low": tok_ci_low,
                "output_tokens_per_second_ci95_high": tok_ci_high,
                "latency_p95_ms_mean": statistics.fmean(p95),
                "success_rate_mean": statistics.fmean(success_rates),
                "measured_acceptance_rate_mean": (
                    statistics.fmean(acceptance_rates) if acceptance_rates else None
                ),
                "measured_reject_rate_mean": (
                    statistics.fmean(reject_rates) if reject_rates else None
                ),
            }
        )

    scenarios_summary.sort(key=lambda item: item["node_count"])
    return {"scenarios": scenarios_summary}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def git_commit() -> str:
    try:
        return (
            subprocess.check_output(["git", "rev-parse", "HEAD"], text=True)
            .strip()
        )
    except Exception:  # noqa: BLE001
        return "unknown"


def write_markdown_report(
    path: Path,
    summary: dict[str, Any],
    meta: dict[str, Any],
) -> None:
    lines = [
        "# Shard Mesh Scale Benchmark Report",
        "",
        f"- Timestamp (UTC): `{meta['timestamp_utc']}`",
        f"- Git commit: `{meta['git_commit']}`",
        f"- Inference mode: `{meta['inference_mode']}`",
        f"- Runs per scenario: `{meta['runs_per_scenario']}`",
        f"- Requests per run: `{meta['requests_per_run']}`",
        f"- Concurrency: `{meta['concurrency']}`",
        "",
        "## Methodology",
        "- Live HTTP requests to `/v1/chat/completions` with `stream=false`.",
        "- Metrics deltas captured from `/metrics/summary` before/after each run.",
        "- No synthetic acceptance/savings values are generated by this tool.",
        "- Scenario order is randomized each cycle to reduce warm-cache bias.",
        "",
        "## Scenario Summary",
        "",
        "| Scenario | Nodes | TPS Mean | TPS 95% CI | Token/s Mean | Token/s 95% CI | p95 Latency (ms) | Success Rate | Acceptance | Reject |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for item in summary["scenarios"]:
        acc = (
            f"{item['measured_acceptance_rate_mean']:.4f}"
            if item["measured_acceptance_rate_mean"] is not None
            else "n/a"
        )
        rej = (
            f"{item['measured_reject_rate_mean']:.4f}"
            if item["measured_reject_rate_mean"] is not None
            else "n/a"
        )
        lines.append(
            "| {scenario} | {node_count} | {tps:.3f} | [{lo:.3f}, {hi:.3f}] | "
            "{tokps:.3f} | [{tok_lo:.3f}, {tok_hi:.3f}] | {p95:.2f} | {succ:.4f} | {acc} | {rej} |".format(
                scenario=item["scenario"],
                node_count=item["node_count"],
                tps=item["throughput_rps_mean"],
                lo=item["throughput_rps_ci95_low"],
                hi=item["throughput_rps_ci95_high"],
                tokps=item["output_tokens_per_second_mean"],
                tok_lo=item["output_tokens_per_second_ci95_low"],
                tok_hi=item["output_tokens_per_second_ci95_high"],
                p95=item["latency_p95_ms_mean"],
                succ=item["success_rate_mean"],
                acc=acc,
                rej=rej,
            )
        )

    lines.extend(
        [
            "",
            "## Investor Note",
            "- Use raw JSON/CSV + manifest hashes for due diligence.",
            "- Claims should reference confidence intervals, not single best runs.",
            "",
        ]
    )
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description="Run real mesh-scale benchmarks")
    parser.add_argument("--scenarios-json", required=True, help="Path to scenarios JSON")
    parser.add_argument("--out-dir", default="benchmarks/results", help="Output directory")
    parser.add_argument("--runs-per-scenario", type=int, default=5)
    parser.add_argument("--requests-per-run", type=int, default=40)
    parser.add_argument("--warmup-requests", type=int, default=5)
    parser.add_argument("--concurrency", type=int, default=8)
    parser.add_argument("--request-timeout-s", type=float, default=45.0)
    parser.add_argument("--inference-mode", default="distributed")
    parser.add_argument("--prompt", default=DEFAULT_PROMPT)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    random.seed(args.seed)
    scenarios = parse_scenarios(Path(args.scenarios_json))
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    run_stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_dir = out_dir / f"mesh-scale-{run_stamp}"
    run_dir.mkdir(parents=True, exist_ok=True)

    run_results: list[dict[str, Any]] = []
    for run_index in range(1, args.runs_per_scenario + 1):
        order = scenarios[:]
        random.shuffle(order)
        for scenario in order:
            result = benchmark_scenario(
                scenario=scenario,
                run_index=run_index,
                requests_per_run=args.requests_per_run,
                concurrency=args.concurrency,
                warmup_requests=args.warmup_requests,
                inference_mode=args.inference_mode,
                request_timeout_s=args.request_timeout_s,
                prompt=args.prompt,
            )
            run_results.append(result)
            print(
                f"[{scenario.name} run {run_index}] "
                f"success={result['success_count']}/{result['requests_total']} "
                f"tps={result['throughput_rps']:.3f} "
                f"p95={result['latency_p95_ms']:.1f}ms"
            )

    raw_json = run_dir / "raw-runs.json"
    raw_json.write_text(json.dumps(run_results, indent=2), encoding="utf-8")
    raw_csv = run_dir / "raw-requests.csv"
    write_csv(raw_csv, run_results)

    summary = summarize(run_results)
    summary_json = run_dir / "summary.json"
    summary_json.write_text(json.dumps(summary, indent=2), encoding="utf-8")

    meta = {
        "timestamp_utc": now_iso(),
        "git_commit": git_commit(),
        "seed": args.seed,
        "scenarios_json": str(Path(args.scenarios_json)),
        "runs_per_scenario": args.runs_per_scenario,
        "requests_per_run": args.requests_per_run,
        "warmup_requests": args.warmup_requests,
        "concurrency": args.concurrency,
        "request_timeout_s": args.request_timeout_s,
        "inference_mode": args.inference_mode,
    }
    metadata_json = run_dir / "metadata.json"
    metadata_json.write_text(json.dumps(meta, indent=2), encoding="utf-8")

    report_md = run_dir / "report.md"
    write_markdown_report(report_md, summary, meta)

    manifest = {
        "timestamp_utc": now_iso(),
        "git_commit": meta["git_commit"],
        "artifacts": [
            {"path": p.name, "sha256": sha256_file(p)}
            for p in [raw_json, raw_csv, summary_json, metadata_json, report_md]
        ],
    }
    manifest_json = run_dir / "manifest.json"
    manifest_json.write_text(json.dumps(manifest, indent=2), encoding="utf-8")

    print(f"Wrote benchmark artifacts to: {run_dir}")
    print(f"Summary: {summary_json}")
    print(f"Report: {report_md}")
    print(f"Manifest: {manifest_json}")


if __name__ == "__main__":
    main()
