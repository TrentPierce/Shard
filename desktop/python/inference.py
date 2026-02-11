"""Hybrid speculative inference loop with real HTTP control-plane client.

The cooperative_generate function runs the local model in a loop, periodically
broadcasting context to the Rust sidecar (which publishes to gossipsub),
and incorporating verified draft tokens from Scout peers.
"""

from __future__ import annotations

import asyncio
import time
from collections.abc import AsyncIterator
from typing import Any, Callable, Awaitable

import httpx


class RustControlPlaneClient:
    """Async HTTP client for the Rust sidecar's control-plane API."""

    def __init__(self, base_url: str = "http://127.0.0.1:9091") -> None:
        self._client = httpx.AsyncClient(base_url=base_url, timeout=2.0)

    async def broadcast_work(
        self, request_id: str, prompt_context: str, min_tokens: int
    ) -> bool:
        try:
            r = await self._client.post(
                "/broadcast-work",
                json={
                    "request_id": request_id,
                    "prompt_context": prompt_context,
                    "min_tokens": min_tokens,
                },
            )
            return r.status_code == 200
        except Exception:
            return False

    async def try_pop_result(self) -> dict[str, Any] | None:
        try:
            r = await self._client.get("/pop-result")
            if r.status_code == 200:
                data = r.json()
                return data.get("result")
        except Exception:
            pass
        return None

    async def health(self) -> dict[str, Any] | None:
        try:
            r = await self._client.get("/health")
            if r.status_code == 200:
                return r.json()
        except Exception:
            pass
        return None

    async def close(self) -> None:
        await self._client.aclose()


async def cooperative_generate(
    *,
    prompt: str,
    local_model_generate: Callable[[list[str], str], Awaitable[str | None]],
    verify_draft: Callable[
        [list[str], list[str]], Awaitable[tuple[list[str], str | None]]
    ],
    control_plane: RustControlPlaneClient,
    max_tokens: int = 256,
) -> AsyncIterator[str]:
    """Hybrid speculative loop.

    - Generate locally for baseline throughput.
    - Every ~50ms broadcast context and accept remote draft candidates.
    - Verify remote drafts and yield only accepted tokens.
    """
    generated: list[str] = []
    request_id = f"req-{int(time.time() * 1000)}"
    last_broadcast = 0.0
    tokens_emitted = 0

    while tokens_emitted < max_tokens:
        # ── local generation ──
        local_token = await local_model_generate(generated, prompt)
        if local_token is None:
            break
        generated.append(local_token)
        yield local_token
        tokens_emitted += 1

        # ── periodic broadcast to swarm ──
        now = time.perf_counter()
        if (now - last_broadcast) >= 0.05:
            context = " ".join(generated[-100:])
            await control_plane.broadcast_work(request_id, context, min_tokens=5)
            last_broadcast = now

        # ── check for scout results ──
        result = await control_plane.try_pop_result()
        if not result:
            continue

        draft_tokens: list[str] = result.get("draft_tokens", [])
        if not draft_tokens:
            continue

        accepted, correction = await verify_draft(generated, draft_tokens)

        for tok in accepted:
            if tokens_emitted >= max_tokens:
                break
            generated.append(tok)
            yield tok
            tokens_emitted += 1

        if correction and tokens_emitted < max_tokens:
            generated.append(correction)
            yield correction
            tokens_emitted += 1
