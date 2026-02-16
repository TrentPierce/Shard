from __future__ import annotations

import importlib
import sys
from pathlib import Path

import pytest

fastapi = pytest.importorskip("fastapi")
TestClient = pytest.importorskip("fastapi.testclient").TestClient

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))


def _load_client():
    module = importlib.import_module("inference_service")
    module = importlib.reload(module)
    return TestClient(module.app), module


def test_inference_service_health_reports_runtime_absent(monkeypatch) -> None:
    monkeypatch.delenv("BITNET_LIB", raising=False)
    monkeypatch.delenv("BITNET_MODEL", raising=False)
    client, _module = _load_client()

    resp = client.get("/health")
    assert resp.status_code == 200
    assert resp.json()["bitnet_loaded"] is False


def test_inference_service_generate_returns_503_without_runtime(monkeypatch) -> None:
    monkeypatch.delenv("BITNET_LIB", raising=False)
    monkeypatch.delenv("BITNET_MODEL", raising=False)
    client, _module = _load_client()

    resp = client.post("/generate", json={"prompt": "hello", "max_tokens": 4})
    assert resp.status_code == 503
