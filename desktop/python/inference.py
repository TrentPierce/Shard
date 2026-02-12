"""Hybrid speculative inference loop with real HTTP control-plane client.

The cooperative_generate function runs the local model in a loop, periodically
broadcasting context to the Rust sidecar (which publishes to gossipsub),
and incorporating verified draft tokens from Scout peers.

Golden Ticket Security:
- Work requests may be Golden Tickets (pre-solved prompts for verification)
- Scout responses to Golden Tickets are verified for correctness
- Scouts that fail Golden Tickets are banned from the network
- Banned scouts are filtered out from draft acceptance
"""

from __future__ import annotations

import logging
import time
from collections.abc import AsyncIterator
from typing import Any, Awaitable, Callable

try:
    import httpx  # type: ignore
except ModuleNotFoundError:  # pragma: no cover - exercised in constrained envs
    httpx = None

# Import Golden Ticket security functions
_is_scout_banned = None
_verify_golden_ticket = None
_maybe_inject_golden_ticket = None
try:
    from golden_ticket import (
        is_scout_banned as _is_scout_banned,
        verify_golden_ticket as _verify_golden_ticket,
        maybe_inject_golden_ticket as _maybe_inject_golden_ticket,
    )
    GOLDEN_TICKET_AVAILABLE = True
except ImportError:
    GOLDEN_TICKET_AVAILABLE = False

LOGGER = logging.getLogger("shard.inference")


class RustControlPlaneClient:
    """Async HTTP client for the Rust sidecar's control-plane API."""

    def __init__(self, base_url: str = "http://127.0.0.1:9091") -> None:
        if httpx is None:
            raise RuntimeError(
                "httpx is required for RustControlPlaneClient. Install desktop/python/requirements.txt"
            )
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
            return None
        return None

    async def health(self) -> dict[str, Any] | None:
        try:
            r = await self._client.get("/health")
            if r.status_code == 200:
                return r.json()
        except Exception:
            return None
        return None

    async def close(self) -> None:
        await self._client.aclose()

    async def submit_draft_result(
        self, work_id: str, scout_id: str, draft_text: str
    ) -> bool:
        """Submit a draft result from a scout to the work queue.
        
        This allows scouts to submit their generated tokens for verification.
        """
        try:
            # Store the result for retrieval by try_pop_result
            # In a full implementation, this would go to Redis or similar
            # For now, we use a simple in-memory queue via HTTP
            r = await self._client.post(
                "/submit-draft",
                json={
                    "work_id": work_id,
                    "scout_id": scout_id,
                    "draft_text": draft_text,
                    "timestamp": time.time(),
                },
            )
            return r.status_code == 200
        except Exception:
            return False


async def cooperative_generate(
    *,
    prompt: str,
    local_model_generate: Callable[[list[str], str, str], Awaitable[str | None]],
    verify_draft: Callable[
        [list[str], list[str]], Awaitable[tuple[list[str], str | None]]
    ],
    control_plane: RustControlPlaneClient,
    max_tokens: int = 256,
) -> AsyncIterator[str]:
    """Hybrid speculative loop with Golden Ticket security.

    - Generate locally for baseline throughput.
    - Every ~50ms broadcast context and accept remote draft candidates.
    - Verify remote drafts and yield only accepted tokens.
    - Filter out results from banned scouts.
    - Check Golden Ticket responses for correctness.
    """
    generated: list[str] = []
    request_id = f"req-{int(time.time() * 1000)}"
    last_broadcast = 0.0
    tokens_emitted = 0

    while tokens_emitted < max_tokens:
        local_token = await local_model_generate(generated, prompt, request_id)
        if local_token is None:
            break
        generated.append(local_token)
        yield local_token
        tokens_emitted += 1

        now = time.perf_counter()
        if (now - last_broadcast) >= 0.05:
            context = " ".join(generated[-100:])
            
            # Potentially inject a Golden Ticket into the work stream
            if GOLDEN_TICKET_AVAILABLE and _maybe_inject_golden_ticket is not None:
                gt_result = _maybe_inject_golden_ticket(context, request_id)
                if isinstance(gt_result, dict) and gt_result.get("is_golden_ticket"):
                    # Use the Golden Ticket prompt instead of normal context
                    gt_prompt = gt_result.get("prompt")
                    if isinstance(gt_prompt, str):
                        context = gt_prompt
                    LOGGER.debug("Injected Golden Ticket into work: %s", request_id)

            await control_plane.broadcast_work(request_id, context, min_tokens=5)
            last_broadcast = now

        result = await control_plane.try_pop_result()
        if not result:
            continue

        # Extract scout ID and check if banned
        scout_id = result.get("scout_id") or result.get("peer_id")
        if scout_id and GOLDEN_TICKET_AVAILABLE and _is_scout_banned is not None:
            if _is_scout_banned(scout_id):
                LOGGER.warning("Ignoring draft from banned scout: %s", scout_id)
                continue

        draft_tokens: list[str] = result.get("draft_tokens", [])
        draft_text = result.get("draft_text", "")

        # Check if this is a Golden Ticket response
        if GOLDEN_TICKET_AVAILABLE and _verify_golden_ticket is not None and draft_text and scout_id:
            gt_verify = _verify_golden_ticket(request_id, scout_id, draft_text)
            if gt_verify is True:
                # Golden Ticket verified successfully - scout is honest
                LOGGER.debug("Golden Ticket verified for scout: %s", scout_id)
            elif gt_verify is False:
                # Golden Ticket failed - scout is dishonest, already banned by verify_golden_ticket
                LOGGER.warning("Golden Ticket failed for scout: %s", scout_id)
                continue
        
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
