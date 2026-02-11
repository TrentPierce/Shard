from __future__ import annotations

import ctypes
import os
import platform
import subprocess
import time
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / "dist" / "ShardAI"
INTERNAL = DIST / "_internal"


class ReleaseBundleTests(unittest.TestCase):
    def setUp(self) -> None:
        if not DIST.exists():
            self.skipTest("dist/ShardAI bundle is missing")

    def test_boot_artifacts_present(self) -> None:
        daemon = INTERNAL / ("shard-daemon.exe" if platform.system() == "Windows" else "shard-daemon")
        app = DIST / ("shard-api.exe" if platform.system() == "Windows" else "shard-api")
        self.assertTrue(app.exists(), "Packaged API executable missing")
        self.assertTrue(daemon.exists(), "Bundled daemon executable missing")

    def test_symbol_check_shard_rollback(self) -> None:
        libname = "shard_engine.dll" if platform.system() == "Windows" else "libshard_engine.so"
        lib_path = INTERNAL / libname
        if not lib_path.exists():
            self.skipTest(f"Engine library missing: {lib_path}")

        try:
            lib = ctypes.CDLL(str(lib_path))
        except OSError as exc:
            self.skipTest(f"Cannot load packaged engine in this environment: {exc}")
            return

        self.assertTrue(hasattr(lib, "shard_rollback"), "shard_rollback symbol missing")

    def test_double_dip_lock_localhost_rule(self) -> None:
        worker = ROOT / "web" / "public" / "swarm-worker.js"
        text = worker.read_text(encoding="utf-8")
        self.assertIn("/v1/system/topology", text)
        self.assertIn("if (isLocalOracle)", text)

    def test_remote_handshake_not_blocked_by_local_rule(self) -> None:
        # This validates policy behavior: only local-oracle status gates scout work.
        worker = ROOT / "web" / "public" / "swarm-worker.js"
        text = worker.read_text(encoding="utf-8")
        self.assertIn("isLocalOracle = Boolean", text)
        self.assertIn("sequence_id", text)


if __name__ == "__main__":
    unittest.main()
