from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))

# Skip entire module if BitNet DLL not available
bitnet = pytest.importorskip("bitnet.ctypes_bridge")
BitNetRuntime = bitnet.BitNetRuntime


def test_token_id_is_deterministic() -> None:
    a = BitNetRuntime.token_id_for_text("hello")
    b = BitNetRuntime.token_id_for_text("hello")
    c = BitNetRuntime.token_id_for_text("world")
    assert a == b
    assert a != c


@pytest.mark.skip(reason="Requires actual DLL loading - use mock in unit tests")
def test_verify_prefix_uses_deterministic_correction_rule() -> None:
    runtime = BitNetRuntime.__new__(BitNetRuntime)
    runtime._abi = "shard"

    sequence = iter(["ok", "expected-correction"])

    def fake_generate_next_token(_generated: list[str]):
        return next(sequence)

    runtime.generate_next_token = fake_generate_next_token  # type: ignore[assignment]

    accepted, correction = runtime.verify_prefix(["context"], ["ok", "wrong"])
    assert accepted == ["ok"]
    assert correction == "expected-correction"
