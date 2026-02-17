from __future__ import annotations

import importlib
import sys
from pathlib import Path

import pytest

fastapi = pytest.importorskip("fastapi")
TestClient = pytest.importorskip("fastapi.testclient").TestClient

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))


def _load_client(
    monkeypatch,
    api_keys="",
    rate_limit="60",
    max_prompt="16000",
    require_api_key: str | None = None,
    scout_rate_limit: str = "120",
):
    monkeypatch.setenv("SHARD_API_KEYS", api_keys)
    monkeypatch.setenv("SHARD_RATE_LIMIT_PER_MINUTE", rate_limit)
    monkeypatch.setenv("SHARD_SCOUT_RATE_LIMIT_PER_MINUTE", scout_rate_limit)
    monkeypatch.setenv("SHARD_MAX_PROMPT_CHARS", max_prompt)
    if require_api_key is None:
        require_api_key = "true" if api_keys else "false"
    monkeypatch.setenv("SHARD_REQUIRE_API_KEY", require_api_key)
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
    module = importlib.import_module("shard_api")
    fixed_time = module.time.time()
    monkeypatch.setattr(module.time, "time", lambda: fixed_time)

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
    assert metrics.headers["content-type"].startswith("text/plain")
    assert "shard_chat_requests_total" in metrics.text


def test_non_streaming_completion_preserves_token_pieces(monkeypatch) -> None:
    client = _load_client(monkeypatch)
    module = importlib.import_module("shard_api")

    async def _fake_generate(**_kwargs):
        for tok in ["Hello", ",", " world", "!"]:
            yield tok

    monkeypatch.setattr(module, "cooperative_generate", _fake_generate)

    resp = client.post("/v1/chat/completions", json=_payload(content="hey"))
    assert resp.status_code == 200
    payload = resp.json()
    assert payload["choices"][0]["message"]["content"] == "Hello, world!"


def test_streaming_completion_does_not_append_spaces(monkeypatch) -> None:
    client = _load_client(monkeypatch)
    module = importlib.import_module("shard_api")

    class _DummyControlPlane:
        def __init__(self, *args, **kwargs):
            pass

        async def close(self):
            return None

    async def _fake_generate(**_kwargs):
        for tok in ["Hello", ",", " world", "!"]:
            yield tok

    monkeypatch.setattr(module, "RustControlPlaneClient", _DummyControlPlane)
    monkeypatch.setattr(module, "cooperative_generate", _fake_generate)

    with client.stream(
        "POST",
        "/v1/chat/completions",
        json={**_payload(content="hey"), "stream": True},
    ) as resp:
        body = "".join(chunk for chunk in resp.iter_text())

    assert resp.status_code == 200
    assert '"delta": {"content": "Hello "}' not in body
    assert '"delta": {"content": "Hello"}' in body


def test_prompt_format_auto_detects_tinyllama_chatml(monkeypatch) -> None:
    monkeypatch.setenv("SHARD_PROMPT_FORMAT", "auto")
    monkeypatch.setenv("BITNET_MODEL", "/home/ubuntu/models/tinyllama.Q2_K.gguf")
    monkeypatch.setenv("SHARD_TESTING", "1")
    module = importlib.import_module("shard_api")
    module = importlib.reload(module)

    assert module._resolved_prompt_format() == "chatml"
    prompt = module._build_chat_prompt([module.Message(role="user", content="Hey")])
    assert prompt == "<|user|>\nHey</s>\n<|assistant|>\n"


def test_latency_profile_endpoint(monkeypatch) -> None:
    client = _load_client(monkeypatch)

    resp = client.get("/v1/metrics/latency_profile")
    assert resp.status_code == 200
    payload = resp.json()
    assert "p2p_latency_ms" in payload
    assert "local_vs_network" in payload


def test_scout_work_endpoint_returns_work_payload(monkeypatch) -> None:
    client = _load_client(monkeypatch)
    module = importlib.import_module("shard_api")

    class _Resp:
        status_code = 200

        @staticmethod
        def json():
            return {
                "work": {
                    "request_id": "req-1",
                    "prompt_context": "hello from queue",
                    "min_tokens": 2,
                }
            }

    class _HttpClient:
        async def get(self, _path: str):
            return _Resp()

    monkeypatch.setattr(module, "_get_http_client", lambda: _HttpClient())

    resp = client.get("/v1/scout/work")
    assert resp.status_code == 200
    payload = resp.json()
    assert payload["workId"] == "req-1"
    assert payload["prompt"]


def test_scout_work_endpoint_returns_204_when_empty(monkeypatch) -> None:
    client = _load_client(monkeypatch)
    module = importlib.import_module("shard_api")

    class _Resp:
        status_code = 200

        @staticmethod
        def json():
            return {"work": None}

    class _HttpClient:
        async def get(self, _path: str):
            return _Resp()

    monkeypatch.setattr(module, "_get_http_client", lambda: _HttpClient())

    resp = client.get("/v1/scout/work")
    assert resp.status_code == 204


def test_scout_work_rate_limit_sets_retry_after(monkeypatch) -> None:
    client = _load_client(monkeypatch, scout_rate_limit="1")
    module = importlib.import_module("shard_api")
    fixed_time = module.time.time()
    monkeypatch.setattr(module.time, "time", lambda: fixed_time)

    class _Resp:
        status_code = 200

        @staticmethod
        def json():
            return {
                "work": {
                    "request_id": "req-rl",
                    "prompt_context": "limited",
                    "min_tokens": 1,
                }
            }

    class _HttpClient:
        async def get(self, _path: str):
            return _Resp()

    monkeypatch.setattr(module, "_get_http_client", lambda: _HttpClient())

    first = client.get("/v1/scout/work")
    assert first.status_code == 200

    second = client.get("/v1/scout/work")
    assert second.status_code == 429
    assert second.json()["detail"] == "Rate limit exceeded"
    assert "Retry-After" in second.headers
