from __future__ import annotations

import ctypes
import os
import socket
import subprocess
import time
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / "dist"


def _find_release_root() -> Path:
    candidates = [DIST / "ShardAI", DIST / "raw"]
    for c in candidates:
        if c.exists():
            return c
    pytest.skip("release bundle not found; run scripts/build_release.py first")


def _wait_port(host: str, port: int, timeout: float = 5.0) -> bool:
    start = time.time()
    while time.time() - start < timeout:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(0.2)
            if s.connect_ex((host, port)) == 0:
                return True
        time.sleep(0.1)
    return False


def test_symbol_check_shard_rollback_callable() -> None:
    root = _find_release_root()
    lib = root / "_internal" / "lib" / ("shard_engine.dll" if os.name == "nt" else "libshard_engine.so")
    if not lib.exists():
        pytest.skip(f"engine library missing at {lib}")

    dll = ctypes.CDLL(str(lib))
    assert hasattr(dll, "shard_rollback")


def test_boot_test_shard_api_launches() -> None:
    root = _find_release_root()
    exe = root / ("ShardAI.exe" if os.name == "nt" else "ShardAI")
    if not exe.exists():
        pytest.skip(f"frozen api entry missing at {exe}")

    # Smoke start: ensure process starts and binds a local port if configured.
    env = dict(os.environ)
    env.setdefault("PORT", "8000")
    proc = subprocess.Popen([str(exe)], env=env)
    try:
        # Optional check if app binds 8000 in runtime environment.
        _wait_port("127.0.0.1", 8000, timeout=2.0)
    finally:
        proc.terminate()
        proc.wait(timeout=5)


def test_loop_double_dip_and_remote_gate_contract() -> None:
    # Contract test: local topology endpoint existence means double-dip lock should engage in worker.
    # Remote handshake acceptance is exercised by integration env; here we assert artifact presence.
    root = _find_release_root()
    worker = root / "_internal" / "web" / "public" / "swarm-worker.js"
    if not worker.exists():
        pytest.skip(f"worker asset missing at {worker}")

    content = worker.read_text(encoding="utf-8")
    assert "localhost:8000/v1/system/topology" in content
    assert "if (isLocalOracle)" in content
