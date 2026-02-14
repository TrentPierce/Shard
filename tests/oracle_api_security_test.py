from __future__ import annotations

import importlib
import sys
from pathlib import Path

import pytest

fastapi = pytest.importorskip("fastapi")
TestClient = pytest.importorskip("fastapi.testclient").TestClient

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))


def _load_client(monkeypatch, api_keys="", rate_limit="60", max_prompt="16000"):
    monkeypatch.setenv("SHARD_API_KEYS", api_keys)
    monkeypatch.setenv("SHARD_RATE_LIMIT_PER_MINUTE", rate_limit)
    monkeypatch.setenv("SHARD_MAX_PROMPT_CHARS", max_prompt)
    # Enable testing mode - uses mock BitNet for tests
    monkeypatch.setenv("SHARD_TESTING", "1")
    # Clear BITNET env vars to trigger mock mode
    monkeypatch.delenv("BITNET_LIB", raising=False)
    monkeypatch.delenv("BITNET_MODEL", raising=False)

    module = importlib.import_module("shard_api")
    module = importlib.reload(module)
    return TestClient(module.app)


def _payload(content: str = "hello") -> dict:
    return {
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": content}],
        "max_tokens": 4,
        "stream": False,
    }


def test_chat_requires_api_key_when_configured(monkeypatch) -> None:
    client = _load_client(monkeypatch, api_keys="secret-1")

    unauthorized = client.post("/v1/chat/completions", json=_payload())
    assert unauthorized.status_code == 401

    authorized = client.post(
        "/v1/chat/completions",
        headers={"X-API-Key": "secret-1"},
        json=_payload(),
    )
    assert authorized.status_code == 200


def test_chat_rate_limit(monkeypatch) -> None:
    client = _load_client(monkeypatch, rate_limit="1")

    first = client.post("/v1/chat/completions", json=_payload())
    assert first.status_code == 200

    second = client.post("/v1/chat/completions", json=_payload())
    assert second.status_code == 429


def test_chat_request_validation(monkeypatch) -> None:
    client = _load_client(monkeypatch)

    invalid_role = client.post(
        "/v1/chat/completions",
        json={
            "messages": [{"role": "hacker", "content": "hi"}],
            "max_tokens": 2,
        },
    )
    assert invalid_role.status_code == 422

    invalid_tokens = client.post(
        "/v1/chat/completions",
        json={
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 999999,
        },
    )
    assert invalid_tokens.status_code == 422


def test_chat_prompt_size_limit(monkeypatch) -> None:
    client = _load_client(monkeypatch, max_prompt="8")

    oversized = client.post("/v1/chat/completions", json=_payload(content="this is too long"))
    assert oversized.status_code == 413


def test_metrics_endpoint(monkeypatch) -> None:
    client = _load_client(monkeypatch)

    metrics = client.get("/metrics")
    assert metrics.status_code == 200
    assert "shard_chat_requests_total" in metrics.text


def test_latency_profile_endpoint(monkeypatch) -> None:
    client = _load_client(monkeypatch)

    resp = client.get("/v1/metrics/latency_profile")
    assert resp.status_code == 200
    payload = resp.json()
    assert "p2p_latency_ms" in payload
    assert "local_vs_network" in payload
