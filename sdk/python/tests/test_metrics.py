from __future__ import annotations

import httpx
import pytest

from shard.errors import ServerError


def test_metrics_summary_success(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/metrics/summary"
        return httpx.Response(200, json={"active_nodes": 3, "healthy_nodes": 2})

    client = make_client(handler)
    summary = client.metrics.summary()
    assert summary.active_nodes == 3


def test_metrics_summary_error(make_client):
    def handler(_: httpx.Request) -> httpx.Response:
        return httpx.Response(500, text="boom")

    client = make_client(handler)
    with pytest.raises(ServerError):
        client.metrics.summary()


def test_webgpu_coverage_edge_nulls(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/metrics/webgpu-coverage"
        return httpx.Response(200, json={"eligible_pct": 0.0})

    client = make_client(handler)
    coverage = client.metrics.webgpu_coverage()
    assert coverage.eligible_pct == 0.0
    assert coverage.total_probes == 0
