from __future__ import annotations

from typing import Any

GATES = [
    ("acceptance_rate_pct", ">=", 65.0, "Draft acceptance rate must be >= 65%"),
    ("latency_vs_baseline_ratio", "<=", 1.5, "p95 latency must be <= 1.5x baseline"),
    ("error_rate_pct", "<=", 1.0, "Error rate must be <= 1%"),
]


def _passes(actual: float, op: str, expected: float) -> bool:
    if op == ">=":
        return actual >= expected
    if op == "<=":
        return actual <= expected
    raise ValueError(f"Unsupported gate operator: {op}")


def evaluate_gates(results: dict[str, Any]) -> tuple[bool, list[str]]:
    failures: list[str] = []

    for field, op, expected, description in GATES:
        value = results.get(field)
        if field == "latency_vs_baseline_ratio" and value is None:
            print("WARNING: skipping latency_vs_baseline_ratio gate (no baseline provided)")
            continue

        if value is None:
            failures.append(f"{description} (missing metric: {field})")
            continue

        actual = float(value)
        if not _passes(actual, op, float(expected)):
            failures.append(f"{description} (actual={actual:.4f}, expected {op} {expected:.4f})")

    return (len(failures) == 0, failures)
