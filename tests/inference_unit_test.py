from __future__ import annotations

import asyncio
import struct
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))
from bitnet.ctypes_bridge import (
    SNAPSHOT_HEADER_SIZE,
    SNAPSHOT_MAGIC,
    SNAPSHOT_VERSION,
    BitNetConfig,
    BitNetRuntime,
)
from inference import KvCheckpointManager, cooperative_generate


class FakeControlPlane:
    def __init__(self, results: list[dict] | None = None) -> None:
        self._results = list(results or [])
        self.broadcasts: list[tuple[str, str, int]] = []

    async def broadcast_work(self, request_id: str, prompt_context: str, min_tokens: int) -> bool:
        self.broadcasts.append((request_id, prompt_context, min_tokens))
        return True

    async def try_pop_result(self):
        if not self._results:
            return None
        return self._results.pop(0)


class FakeKvRuntime:
    def __init__(self) -> None:
        self.snapshots_exported = 0
        self.snapshots_imported = 0

    def export_kv_snapshot(self) -> bytes:
        self.snapshots_exported += 1
        payload = b"state" + bytes([self.snapshots_exported])
        header = struct.pack(
            "<IIII",
            SNAPSHOT_MAGIC,
            SNAPSHOT_VERSION,
            self.snapshots_exported,
            len(payload),
        )
        return header + payload

    def import_kv_snapshot(self, snapshot: bytes | bytearray | memoryview) -> None:
        magic, version, _npast, payload_size = struct.unpack_from("<IIII", snapshot, 0)
        assert magic == SNAPSHOT_MAGIC
        assert version == SNAPSHOT_VERSION
        assert SNAPSHOT_HEADER_SIZE + payload_size == len(snapshot)
        self.snapshots_imported += 1


def test_kv_checkpoint_manager_periodic_capture_and_restore() -> None:
    runtime = FakeKvRuntime()
    manager = KvCheckpointManager(runtime=runtime, every_n_tokens=2)

    assert manager.latest is None
    manager.maybe_checkpoint(["a"], emitted_tokens=1)
    assert manager.latest is None

    cp = manager.maybe_checkpoint(["a", "b"], emitted_tokens=2)
    assert cp is not None
    assert runtime.snapshots_exported == 1

    restored = manager.restore_latest()
    assert restored is not None
    assert runtime.snapshots_imported == 1


def test_cooperative_generate_emits_local_then_verified_remote_tokens() -> None:
    async def runner() -> list[str]:
        local_tokens = iter(["hello", "from", "shard", None])

        async def local_model_generate(_generated: list[str], _prompt: str, _request_id: str):
            return next(local_tokens)

        async def verify_draft(_generated: list[str], draft: list[str]):
            if draft == ["scout-a", "scout-b"]:
                return ["scout-a"], "shard-c"
            return draft, None

        control = FakeControlPlane(results=[{"draft_tokens": ["scout-a", "scout-b"]}])

        emitted = []
        async for tok in cooperative_generate(
            prompt="test",
            local_model_generate=local_model_generate,
            verify_draft=verify_draft,
            control_plane=control,  # type: ignore[arg-type]
            max_tokens=6,
        ):
            emitted.append(tok)

        assert control.broadcasts
        return emitted

    emitted = asyncio.run(runner())
    assert emitted == ["hello", "scout-a", "shard-c", "from", "shard"]


def test_cooperative_generate_stops_on_local_none() -> None:
    async def runner() -> list[str]:
        async def local_model_generate(_generated: list[str], _prompt: str, _request_id: str):
            return None

        async def verify_draft(_generated: list[str], draft: list[str]):
            return draft, None

        control = FakeControlPlane()

        emitted = []
        async for tok in cooperative_generate(
            prompt="test",
            local_model_generate=local_model_generate,
            verify_draft=verify_draft,
            control_plane=control,  # type: ignore[arg-type]
            max_tokens=3,
        ):
            emitted.append(tok)

        return emitted

    emitted = asyncio.run(runner())
    assert emitted == []


def test_cooperative_generate_uses_kv_checkpoint_manager_on_remote_gap() -> None:
    async def runner() -> tuple[list[str], FakeKvRuntime]:
        local_tokens = iter(["tok1", "tok2", "tok3", None])

        async def local_model_generate(_generated: list[str], _prompt: str, _request_id: str):
            return next(local_tokens)

        async def verify_draft(_generated: list[str], draft: list[str]):
            return draft, None

        control = FakeControlPlane(results=[])
        runtime = FakeKvRuntime()
        manager = KvCheckpointManager(runtime=runtime, every_n_tokens=1)

        emitted = []
        async for tok in cooperative_generate(
            prompt="test",
            local_model_generate=local_model_generate,
            verify_draft=verify_draft,
            control_plane=control,  # type: ignore[arg-type]
            max_tokens=3,
            kv_checkpoint_manager=manager,
        ):
            emitted.append(tok)

        return emitted, runtime

    emitted, runtime = asyncio.run(runner())
    assert emitted == ["tok1", "tok2", "tok3"]
    assert runtime.snapshots_exported >= 1
    assert runtime.snapshots_imported >= 1


def test_bitnet_runtime_import_kv_snapshot_rejects_invalid_header() -> None:
    runtime = BitNetRuntime.__new__(BitNetRuntime)
    runtime._abi = "shard"
    runtime._cfg = BitNetConfig(lib_path="x", model_path="y", max_kv_snapshot_bytes=1024)

    class Lib:
        def shard_kv_snapshot_import(self, *_args):
            return 0

        def shard_free(self, *_args):
            return None

    runtime._lib = Lib()
    runtime._handle = object()

    bad = struct.pack("<IIII", 123, SNAPSHOT_VERSION, 0, 0)
    try:
        runtime.import_kv_snapshot(bad)
        raised = False
    except ValueError:
        raised = True
    assert raised
