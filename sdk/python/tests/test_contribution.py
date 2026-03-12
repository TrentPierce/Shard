from __future__ import annotations

import json

import httpx


def test_create_contribution_session(make_client):
    client = make_client(lambda request: httpx.Response(200, json={"ok": True}, request=request))
    session = client.contribution.create_session()

    assert len(session.public_key_hex) == 64
    assert len(session.seed_hex) == 64


def test_register_node_posts_signed_envelope(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/signed/register-node"
        payload = json.loads(request.content.decode("utf-8"))
        envelope = payload["envelope"]
        assert envelope["payload"]["node_pubkey"] == envelope["signer_pubkey_hex"]
        assert envelope["payload"]["role"] == "verifier"
        assert envelope["payload"]["capacity"] == 2
        assert envelope["payload_hash_hex"]
        assert envelope["signature_hex"]
        return httpx.Response(200, json={"ok": True, "detail": "registered"}, request=request)

    client = make_client(handler)
    session = client.contribution.create_session(
        seed_hex="01" * 32,
        nonce_seed=100,
    )
    ack = session.register_node(role="verifier", capacity=2, timestamp_ms=1234)

    assert ack.ok is True
    assert ack.detail == "registered"


def test_heartbeat_and_metrics_report_use_signed_contribution_paths(make_client):
    seen_paths: list[str] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen_paths.append(request.url.path)
        payload = json.loads(request.content.decode("utf-8"))
        envelope = payload["envelope"]
        assert envelope["payload"]["node_pubkey"] == envelope["signer_pubkey_hex"]
        return httpx.Response(200, json={"ok": True, "detail": "accepted"}, request=request)

    client = make_client(handler)
    session = client.contribution.create_session(
        seed_hex="02" * 32,
        nonce_seed=200,
    )

    heartbeat = session.heartbeat(
        role="verifier",
        queue_depth=1,
        node_latency_ms=40,
        uptime_seconds=120,
        capability_tier="gpu_fast",
        gpu_available=True,
        public_api=True,
        public_api_addr="http://127.0.0.1:9091",
        timestamp_ms=2222,
    )
    metrics = session.report_metrics(
        role="verifier",
        queue_depth=2,
        node_latency_ms=44,
        uptime_seconds=180,
        capability_tier="gpu_fast",
        gpu_available=True,
        public_api=True,
        timestamp_ms=3333,
    )

    assert heartbeat.ok is True
    assert metrics.ok is True
    assert seen_paths == ["/signed/heartbeat", "/signed/metrics-report"]


def test_deregister_node_posts_signed_request(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/signed/deregister-node"
        payload = json.loads(request.content.decode("utf-8"))
        envelope = payload["envelope"]
        assert envelope["payload"]["role"] == "verifier"
        return httpx.Response(200, json={"ok": True, "detail": "deregistered"}, request=request)

    client = make_client(handler)
    session = client.contribution.create_session(
        seed_hex="03" * 32,
        nonce_seed=300,
    )
    ack = session.deregister_node(role="verifier", timestamp_ms=4444)

    assert ack.ok is True
    assert ack.detail == "deregistered"
