from __future__ import annotations

import httpx
import pytest

from shard.errors import MeshDegradedError


def test_mesh_topology_success(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/topology"
        return httpx.Response(200, json={"status": "ok", "known_peer_count": 2})

    client = make_client(handler)
    topo = client.mesh.topology()
    assert topo.status == "ok"


def test_mesh_topology_error(make_client):
    def handler(_: httpx.Request) -> httpx.Response:
        return httpx.Response(503, text="mesh_degraded")

    client = make_client(handler)
    with pytest.raises(MeshDegradedError):
        client.mesh.topology()


def test_mesh_peers_edge_empty(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/peers"
        return httpx.Response(200, json={"peers": []})

    client = make_client(handler)
    peers = client.mesh.peers()
    assert peers == []
