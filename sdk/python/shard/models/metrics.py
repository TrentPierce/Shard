from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class MetricsSummary(BaseModel):
    model_config = ConfigDict(extra="ignore")
    active_nodes: int | None = None
    healthy_nodes: int | None = None
    average_latency_ms: float | None = None
    p95_latency_ms: float | None = None


class WebGPUCoverage(BaseModel):
    model_config = ConfigDict(extra="ignore")
    total_probes: int = 0
    eligible_pct: float = 0.0
    high_performance_pct: float = 0.0
    low_power_pct: float = 0.0
    ineligible_pct: float = 0.0
    ineligible_reason_breakdown: dict[str, float] = {}
    browser_breakdown: dict[str, float] = {}
    os_breakdown: dict[str, float] = {}
