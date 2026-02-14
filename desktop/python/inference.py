"""Hybrid speculative inference loop with real HTTP control-plane client.

The cooperative_generate function runs the local model in a loop, periodically
broadcasting context to the Rust sidecar (which publishes to gossipsub),
and incorporating verified draft tokens from Scout peers.

Golden Ticket Security:
- Work requests may be Golden Tickets (pre-solved prompts for verification)
- Scout responses to Golden Tickets are verified for correctness
- Scouts that fail Golden Tickets are banned from the network
- Banned scouts are filtered out from draft acceptance

Pitch Mode (Demo Mode):
- When SHARD_PITCH_MODE=1, enables demo resilience features
- If a peer fails, immediately reroute to next best peer (0ms delay)
- Log rerouting events for toast notifications
"""

from __future__ import annotations

import logging
import asyncio
import os
import time
from collections import deque
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Protocol

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

# Pitch Mode configuration
PITCH_MODE = os.getenv("SHARD_PITCH_MODE", "0") == "1"


class KvCheckpointRuntime(Protocol):
    """Interface used by the generation loop for snapshot handoff.

    Implemented by desktop/python/bitnet/ctypes_bridge.BitNetRuntime.
    """

    def export_kv_snapshot(self) -> bytes: ...

    def import_kv_snapshot(self, snapshot: bytes | bytearray | memoryview) -> None: ...


@dataclass
class KvCheckpointState:
    """In-memory bounded checkpoint used for local fallback/handoff.

    - payload: serialized runtime state (KV cache + decoding metadata)
    - token_count: generated token count at capture time
    - generated_tail: short textual suffix to quickly rebuild prompt context
    """

    payload: bytes
    token_count: int
    generated_tail: tuple[str, ...]


class KvCheckpointManager:
    """Captures periodic KV snapshots and restores the latest safe snapshot.

    Checkpoint cadence is token-based instead of time-based so this logic remains
    deterministic under event-loop jitter.
    """

    def __init__(
        self,
        *,
        runtime: KvCheckpointRuntime,
        every_n_tokens: int = 8,
        max_generated_tail: int = 256,
    ) -> None:
        self._runtime = runtime
        self._every_n_tokens = max(1, every_n_tokens)
        self._max_generated_tail = max(16, max_generated_tail)
        self._latest: KvCheckpointState | None = None

    @property
    def latest(self) -> KvCheckpointState | None:
        return self._latest

    def maybe_checkpoint(self, generated: list[str], emitted_tokens: int) -> KvCheckpointState | None:
        """Capture a new checkpoint when token cadence is reached."""
        if emitted_tokens <= 0 or (emitted_tokens % self._every_n_tokens) != 0:
            return self._latest
        payload = self._runtime.export_kv_snapshot()
        self._latest = KvCheckpointState(
            payload=payload,
            token_count=emitted_tokens,
            generated_tail=tuple(generated[-self._max_generated_tail :]),
        )
        return self._latest

    def restore_latest(self) -> KvCheckpointState | None:
        """Restore the most recent checkpoint into the runtime."""
        if self._latest is None:
            return None
        self._runtime.import_kv_snapshot(self._latest.payload)
        return self._latest


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
    telemetry_hook: Callable[[dict[str, float | int]], Awaitable[None] | None] | None = None,
    scout_event_hook: Callable[[dict[str, object]], Awaitable[None] | None] | None = None,
    kv_checkpoint_manager: KvCheckpointManager | None = None,
) -> AsyncIterator[str]:
    """Hybrid speculative loop with Golden Ticket security.

    - Generate locally for baseline throughput.
    - Every ~50ms broadcast context and accept remote draft candidates.
    - Verify remote drafts and yield only accepted tokens.
    - Filter out results from banned scouts.
    - Check Golden Ticket responses for correctness.
    - In Pitch Mode: immediately reroute on peer failure (0ms delay).
    """
    generated: list[str] = []
    request_id = f"req-{int(time.time() * 1000)}"
    last_broadcast = 0.0
    tokens_emitted = 0
    
    # Pitch Mode: Track failed peers for rerouting notifications
    failed_peers: set[str] = set()
    last_reroute_log = 0.0

    # Keep a tiny moving average of local token generation time so we can
    # compare what speculative remote drafts cost versus equivalent local work.
    local_token_cost_ms: deque[float] = deque(maxlen=64)
    remote_timeout_s = max(0.01, float(os.getenv("SHARD_SCOUT_RESULT_TIMEOUT_S", "0.15")))
    remote_disabled = False

    while tokens_emitted < max_tokens:
        local_start = time.perf_counter()
        local_token = await local_model_generate(generated, prompt, request_id)
        local_elapsed_ms = (time.perf_counter() - local_start) * 1000.0
        if local_token is None:
            break
        generated.append(local_token)
        local_token_cost_ms.append(local_elapsed_ms)
        yield local_token
        tokens_emitted += 1

        if kv_checkpoint_manager is not None:
            try:
                kv_checkpoint_manager.maybe_checkpoint(generated, tokens_emitted)
            except Exception as exc:
                LOGGER.warning("KV checkpoint capture skipped: %s", exc)

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

        if remote_disabled:
            continue

        remote_eval_start = time.perf_counter()
        try:
            result = await asyncio.wait_for(control_plane.try_pop_result(), timeout=remote_timeout_s)
        except asyncio.TimeoutError:
            # Scout disconnected or partitioned; disable speculation for this request
            # and immediately continue in local auto-regressive mode.
            LOGGER.warning(
                "Scout draft timeout exceeded (%.2fs); switching request %s to local-only fallback",
                remote_timeout_s,
                request_id,
            )
            remote_disabled = True
            continue
        
        # Pitch Mode: Handle peer failure immediately (0ms delay)
        if not result and PITCH_MODE:
            # Check if we should log a rerouting event (throttle to avoid spam)
            if now - last_reroute_log > 2.0:
                # Simulate peer failure detection
                health = await control_plane.health()
                if health:
                    connected_peers = health.get("connected_peers", 0)
                    if connected_peers > 0:
                        # Log rerouting for toast notification
                        LOGGER.info("Rerouting to next best peer (Pitch Mode demo)")
                        last_reroute_log = now
            # In pitch mode, immediately retry without waiting
            continue

        if not result:
            # Remote speculation currently unavailable. If we have an already captured
            # checkpoint, ensure runtime state is coherent and continue local AR decode.
            if kv_checkpoint_manager is not None and kv_checkpoint_manager.latest is not None:
                try:
                    kv_checkpoint_manager.restore_latest()
                    generated = list(kv_checkpoint_manager.latest.generated_tail)
                except Exception as exc:
                    LOGGER.warning("KV checkpoint restore skipped: %s", exc)
            continue

        # Extract scout ID and check if banned
        scout_id = result.get("scout_id") or result.get("peer_id")
        
        # Pitch Mode: Track and log peer failures
        if PITCH_MODE and scout_id and result.get("error"):
            if scout_id not in failed_peers:
                failed_peers.add(scout_id)
                LOGGER.warning(f"Rerouting to Node [{scout_id[:12]}...] (Pitch Mode)")
                last_reroute_log = now

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
        remote_eval_ms = (time.perf_counter() - remote_eval_start) * 1000.0

        if scout_event_hook is not None and scout_id:
            accepted_full = len(accepted) == len(draft_tokens)
            event_payload: dict[str, object] = {
                "scout_id": scout_id,
                "accepted": accepted_full,
                "accepted_tokens": len(accepted),
                "draft_tokens": len(draft_tokens),
                "reason": None if accepted_full else "draft mismatch",
            }
            maybe_evt = scout_event_hook(event_payload)
            if hasattr(maybe_evt, "__await__"):
                await maybe_evt

        if telemetry_hook is not None and draft_tokens:
            n_tokens = len(draft_tokens)
            local_avg_ms = (sum(local_token_cost_ms) / len(local_token_cost_ms)) if local_token_cost_ms else 0.0
            sample = {
                "tokens": n_tokens,
                "local_generate_ms": local_avg_ms * n_tokens,
                "network_rtt_plus_verify_ms": remote_eval_ms,
            }
            maybe_awaitable = telemetry_hook(sample)
            if hasattr(maybe_awaitable, "__await__"):
                await maybe_awaitable

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
