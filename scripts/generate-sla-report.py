#!/usr/bin/env python3
"""
Weekly SLA Report Generator
Usage: python scripts/generate-sla-report.py --prometheus http://localhost:9095 --out sla-report.md
"""

from __future__ import annotations

import argparse
import datetime as dt
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import httpx


@dataclass
class Series:
    metric: dict[str, str]
    values: list[tuple[float, float]]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate weekly Shard SLA markdown report")
    parser.add_argument("--prometheus", required=True, help="Prometheus base URL")
    parser.add_argument("--out", required=True, help="Output markdown file path")
    parser.add_argument("--step", default="1h", help="Prometheus step interval")
    return parser.parse_args()


def parse_series(payload: dict[str, Any]) -> list[Series]:
    result = payload.get("data", {}).get("result", [])
    parsed: list[Series] = []
    for item in result:
        metric = item.get("metric", {})
        values = []
        for raw_ts, raw_val in item.get("values", []):
            try:
                ts = float(raw_ts)
                val = float(raw_val)
            except (TypeError, ValueError):
                continue
            values.append((ts, val))
        parsed.append(Series(metric=metric, values=values))
    return parsed


def series_latest(series: list[Series]) -> float:
    latest = 0.0
    for s in series:
        if s.values:
            latest = s.values[-1][1]
    return latest


def flatten_values(series: list[Series]) -> list[tuple[float, float]]:
    merged: list[tuple[float, float]] = []
    for s in series:
        merged.extend(s.values)
    return sorted(merged, key=lambda x: x[0])


def status_line(name: str, value: float, threshold_desc: str, passed: bool) -> str:
    status = "PASS" if passed else "FAIL"
    return f"| {name} | {value:.4f} | {threshold_desc} | {status} |"


def summarize_incidents(label: str, values: list[tuple[float, float]], predicate) -> list[str]:
    incidents: list[str] = []
    for ts, val in values:
        if predicate(val):
            dt_s = dt.datetime.fromtimestamp(ts, tz=dt.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
            incidents.append(f"- {label} breach at {dt_s}: {val:.4f}")
    return incidents


def query_range(
    client: httpx.Client,
    base_url: str,
    query: str,
    start_ts: int,
    end_ts: int,
    step: str,
) -> list[Series]:
    resp = client.get(
        f"{base_url.rstrip('/')}/api/v1/query_range",
        params={"query": query, "start": start_ts, "end": end_ts, "step": step},
        timeout=60.0,
    )
    resp.raise_for_status()
    payload = resp.json()
    if payload.get("status") != "success":
        raise RuntimeError(f"Prometheus query failed for '{query}': {payload}")
    return parse_series(payload)


def main() -> None:
    args = parse_args()
    end = dt.datetime.now(tz=dt.timezone.utc)
    start = end - dt.timedelta(days=7)
    start_ts = int(start.timestamp())
    end_ts = int(end.timestamp())

    queries = {
        "sla_breaches": "shard_sla_breach_total",
        "weekly_p95_ms": "histogram_quantile(0.95, rate(shard_request_duration_ms_bucket[7d]))",
        "bootstrap_connected": "shard_bootstrap_connected_count",
        "acceptance_rate": "shard_drafts_accepted_total / shard_drafts_submitted_total",
        "routing_split_primary": "sum(rate(overflow_requests_total{destination='primary'}[5m])) / sum(rate(overflow_requests_total[5m]))",
        "routing_split_shard": "sum(rate(overflow_requests_total{destination='shard'}[5m])) / sum(rate(overflow_requests_total[5m]))",
    }

    with httpx.Client() as client:
        data = {
            key: query_range(client, args.prometheus, expr, start_ts, end_ts, args.step)
            for key, expr in queries.items()
        }

    breach_total = series_latest(data["sla_breaches"])
    weekly_p95 = series_latest(data["weekly_p95_ms"])
    acceptance_rate = series_latest(data["acceptance_rate"])
    primary_split = series_latest(data["routing_split_primary"])
    shard_split = series_latest(data["routing_split_shard"])
    bootstrap_values = flatten_values(data["bootstrap_connected"])
    healthy_points = sum(1 for _, val in bootstrap_values if val >= 2.0)
    uptime_ratio = (healthy_points / len(bootstrap_values)) if bootstrap_values else 0.0

    sla_rows = [
        status_line("SLA Breaches", breach_total, "== 0 over 7d", breach_total == 0.0),
        status_line("Weekly p95 (ms)", weekly_p95, "<= 3000", weekly_p95 <= 3000.0),
        status_line("Bootstrap Uptime Ratio", uptime_ratio, ">= 0.995", uptime_ratio >= 0.995),
        status_line("Acceptance Rate", acceptance_rate, ">= 0.65", acceptance_rate >= 0.65),
    ]

    incidents = []
    incidents.extend(
        summarize_incidents("p95 latency", flatten_values(data["weekly_p95_ms"]), lambda v: v > 3000.0)
    )
    incidents.extend(
        summarize_incidents("acceptance rate", flatten_values(data["acceptance_rate"]), lambda v: v < 0.65)
    )
    incidents.extend(
        summarize_incidents("bootstrap connected count", bootstrap_values, lambda v: v < 2.0)
    )
    if not incidents:
        incidents = ["- No SLA incidents detected in this reporting window."]

    recommendations = []
    if breach_total > 0:
        recommendations.append("- Investigate Shard latency spikes and reduce circuit breaker open events.")
    if weekly_p95 > 3000.0:
        recommendations.append("- Scale verifier capacity or reduce per-request max token defaults.")
    if acceptance_rate < 0.65:
        recommendations.append("- Recalibrate draft acceptance threshold and audit low-quality scout cohorts.")
    if uptime_ratio < 0.995:
        recommendations.append("- Increase bootstrap redundancy and alert on subnet-level reachability early.")
    if not recommendations:
        recommendations = ["- No corrective action required. Continue weekly monitoring cadence."]

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        "\n".join(
            [
                "# Weekly SLA Report",
                "",
                f"- Window Start (UTC): {start.strftime('%Y-%m-%d %H:%M:%S')}",
                f"- Window End (UTC): {end.strftime('%Y-%m-%d %H:%M:%S')}",
                f"- Prometheus: {args.prometheus}",
                "",
                "## SLA Status",
                "",
                "| Metric | Value | Target | Status |",
                "|---|---:|---|---|",
                *sla_rows,
                "",
                "## Time-Series Summary",
                "",
                f"- Routing split (primary): {primary_split:.4f}",
                f"- Routing split (shard): {shard_split:.4f}",
                f"- Bootstrap healthy samples ratio: {uptime_ratio:.4f}",
                "",
                "## Notable Incidents",
                "",
                *incidents,
                "",
                "## Recommendations",
                "",
                *recommendations,
                "",
            ]
        ),
        encoding="utf-8",
    )
    print(f"Wrote SLA report to {out}")


if __name__ == "__main__":
    main()
