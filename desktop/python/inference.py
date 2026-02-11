from __future__ import annotations

import asyncio
import time
from collections.abc import AsyncIterator


class RustControlPlaneClient:
    """Thin async shim.

    Real implementation should call gRPC over UDS `/tmp/shard-control.sock`.
    """

    async def broadcast_work(self, request_id: str, prompt_context: str, min_tokens: int) -> None:
        _ = (request_id, prompt_context, min_tokens)

    async def try_pop_result(self) -> dict | None:
        return None


async def cooperative_generate(
    *,
    prompt: str,
    local_model_generate,
    verify_draft,
    control_plane: RustControlPlaneClient,
) -> AsyncIterator[str]:
    """Hybrid speculative loop.

    - Generate locally for baseline throughput.
    - Every ~50ms broadcast context and accept remote draft candidates.
    - Verify remote drafts and yield only accepted tokens.
    """
    generated: list[str] = []
    request_id = f"req-{int(time.time() * 1000)}"
    last_broadcast = 0.0

    while True:
        local_token = await local_model_generate(generated, prompt)
        if local_token is None:
            break
        generated.append(local_token)
        yield local_token

        now = time.perf_counter()
        if (now - last_broadcast) >= 0.05:
            context = " ".join(generated[-100:])
            await control_plane.broadcast_work(request_id, context, min_tokens=5)
            last_broadcast = now

        result = await control_plane.try_pop_result()
        if not result:
            continue

        draft_tokens: list[str] = result.get("draft_tokens", [])
        accepted, correction = await verify_draft(generated, draft_tokens)

        for tok in accepted:
            generated.append(tok)
            yield tok

        if correction:
            generated.append(correction)
            yield correction
