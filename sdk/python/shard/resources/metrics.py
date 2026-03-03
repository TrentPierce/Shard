from __future__ import annotations

from typing import TYPE_CHECKING

from shard.models.metrics import MetricsSummary, WebGPUCoverage

if TYPE_CHECKING:
    from shard.client import Client


class MetricsResource:
    def __init__(self, client: "Client") -> None:
        self._client = client

    def summary(self) -> MetricsSummary:
        """Return summarized operations metrics."""
        response = self._client._request("GET", "/metrics/summary")
        return MetricsSummary.model_validate(response.json())

    def latency_profile(self) -> dict:
        """Return latency profile percentiles."""
        response = self._client._request("GET", "/metrics/latency-profile")
        return response.json()

    def webgpu_coverage(self) -> WebGPUCoverage:
        """Return aggregated WebGPU coverage telemetry."""
        response = self._client._request("GET", "/metrics/webgpu-coverage")
        return WebGPUCoverage.model_validate(response.json())

    def prometheus(self) -> str:
        """Return raw Prometheus metrics exposition text."""
        response = self._client._request("GET", "/metrics")
        return response.text
