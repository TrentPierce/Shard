from __future__ import annotations

import json
from typing import Any

from benchmarks.gates import GATES, evaluate_gates


class ReportGenerator:
    def __init__(self, results: dict[str, Any]) -> None:
        self.results = results

    def _gate_status_for_metric(self, metric_name: str) -> str:
        if metric_name == "latency_vs_baseline_ratio" and self.results.get(metric_name) is None:
            return "SKIPPED"

        for field, op, expected, _ in GATES:
            if field != metric_name:
                continue
            value = self.results.get(metric_name)
            if value is None:
                return "FAIL"
            numeric = float(value)
            if (op == ">=" and numeric >= expected) or (op == "<=" and numeric <= expected):
                return "PASS"
            return "FAIL"

        return "N/A"

    def render(self) -> str:
        passed, failures = evaluate_gates(self.results)

        metrics_to_show = [
            "total_requests",
            "total_errors",
            "error_rate_pct",
            "acceptance_rate_pct",
            "latency_p50_ms",
            "latency_p95_ms",
            "latency_p99_ms",
            "tokens_per_second",
            "latency_vs_baseline_ratio",
            "network_overhead_bytes_per_request",
        ]

        header = [
            "# Shard Benchmark Report",
            "",
            f"- Timestamp: `{self.results.get('timestamp_utc', 'unknown')}`",
            f"- Scout count: `{self.results.get('scout_count', 'unknown')}`",
            f"- Duration (s): `{self.results.get('duration_seconds', 'unknown')}`",
            f"- Base URL: `{self.results.get('base_url', 'unknown')}`",
            "",
            "## Results",
            "",
            "| Metric | Value | Gate Status |",
            "|---|---:|---|",
        ]

        rows: list[str] = []
        for metric in metrics_to_show:
            value = self.results.get(metric)
            rows.append(f"| `{metric}` | `{value}` | `{self._gate_status_for_metric(metric)}` |")

        gate_section = ["", "## Gate Outcome", ""]
        if passed:
            gate_section.append("ALL GATES PASSED")
        else:
            gate_section.append("Gate failures:")
            for failure in failures:
                gate_section.append(f"- {failure}")

        appendix = [
            "",
            "## Raw JSON Appendix",
            "",
            "```json",
            json.dumps(self.results, indent=2, sort_keys=True),
            "```",
            "",
        ]

        return "\n".join(header + rows + gate_section + appendix)
