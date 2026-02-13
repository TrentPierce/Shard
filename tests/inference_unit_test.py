from __future__ import annotations

import asyncio
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parents[1] / "desktop" / "python"))
from inference import cooperative_generate


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
