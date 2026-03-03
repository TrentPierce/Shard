from __future__ import annotations

import httpx
import pytest

from shard.errors import AuthenticationError


def test_node_status_success(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/node/status"
        return httpx.Response(200, json={"ok": True, "health_status": "ready"})

    client = make_client(handler)
    status = client.node.status()
    assert status.ok is True


def test_node_status_error(make_client):
    def handler(_: httpx.Request) -> httpx.Response:
        return httpx.Response(401, text="unauthorized")

    client = make_client(handler)
    with pytest.raises(AuthenticationError):
        client.node.status()


def test_node_logs_edge_empty(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/node/logs"
        return httpx.Response(200, json={"ok": True, "logs": []})

    client = make_client(handler)
    logs = client.node.logs()
    assert logs == []
