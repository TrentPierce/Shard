from __future__ import annotations

import importlib
import sys
from pathlib import Path

import pytest

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))


def test_control_plane_client_errors_cleanly_without_httpx(monkeypatch: pytest.MonkeyPatch) -> None:
    inference = importlib.import_module("inference")
    monkeypatch.setattr(inference, "httpx", None)

    with pytest.raises(RuntimeError, match="httpx is required"):
        inference.RustControlPlaneClient()
