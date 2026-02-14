import asyncio
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PYTHON_DIR = ROOT / "desktop" / "python"
if str(PYTHON_DIR) not in sys.path:
    sys.path.insert(0, str(PYTHON_DIR))

import oracle_api  # noqa: E402


class _DummyRuntime:
    pass


def test_useful_compute_verifier_accepts_improbable_full_match(monkeypatch):
    async def fake_load():
        return _DummyRuntime()

    async def fake_verify(_generated, draft):
        return draft, None

    monkeypatch.setattr(oracle_api, "get_or_load_bitnet", fake_load)
    monkeypatch.setattr(oracle_api, "_verify_draft", fake_verify)

    verifier = oracle_api.UsefulComputeVerifier(top_k=8, max_tokens=32, failure_threshold=1e-6)
    result = asyncio.run(verifier.verify(["hello"], ["a", "b", "c", "d", "e", "f", "g"]))

    assert result["accepted"] is True
    assert result["probability_bound"] < 1e-6


def test_useful_compute_verifier_rejects_mismatch(monkeypatch):
    async def fake_load():
        return _DummyRuntime()

    async def fake_verify(_generated, _draft):
        return ["a", "b"], "c"

    monkeypatch.setattr(oracle_api, "get_or_load_bitnet", fake_load)
    monkeypatch.setattr(oracle_api, "_verify_draft", fake_verify)

    verifier = oracle_api.UsefulComputeVerifier(top_k=8, max_tokens=32, failure_threshold=1e-12)
    result = asyncio.run(verifier.verify(["hello"], ["a", "b", "x"]))

    assert result["accepted"] is False
    assert result["expected"] == "c"
