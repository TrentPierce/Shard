from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Any


EOS_TOKEN = "<eos>"


@dataclass
class DraftBatch:
    request_id: str
    sequence_id: int
    tokens: list[str]
    peer_id: str = "unknown"


class RustControlPlaneClient:
    """Thin async shim.

    Real implementation should call gRPC over UDS `/tmp/shard-control.sock`.
    """

    async def broadcast_work(
        self,
        *,
        request_id: str,
        context_window: list[str],
        sequence_id: int,
        min_draft_len: int,
    ) -> None:
        _ = (request_id, context_window, sequence_id, min_draft_len)


class FuzzyDraftVerifier:
    """Soft verification for quantization mismatch.

    Accept scout token if it is in Oracle top-k (default top-3) rather than exact-only.
    """

    def __init__(self, model_runtime: Any, k: int = 3) -> None:
        self.model = model_runtime
        self.k = k

    async def verify_draft_batch(self, current_context: list[str], draft_tokens: list[str]) -> list[str]:
        _ = current_context
        accepted_tokens: list[str] = []

        token_ids = [self._to_id(tok) for tok in draft_tokens]
        self.model.eval_tokens(token_ids)

        for i, token_id in enumerate(token_ids):
            top_k = self.model.get_top_k_tokens(k=self.k)
            if token_id in top_k:
                accepted_tokens.append(self._from_id(token_id))
                self.model.commit_token(token_id)
                continue

            correct_token = self.model.sample_token()
            accepted_tokens.append(self._from_id(correct_token))
            self.model.rollback(len(token_ids) - i)
            break

        return accepted_tokens

    @staticmethod
    def _to_id(token: str) -> int:
        # Placeholder encoding shim for scaffold wiring.
        return abs(hash(token)) % 32000

    @staticmethod
    def _from_id(token_id: int) -> str:
        # Placeholder decoding shim for scaffold wiring.
        return f"tok_{token_id}"


class CooperativeGenerator:
    def __init__(
        self,
        *,
        grpc_client: RustControlPlaneClient,
        bid_queue: asyncio.Queue[DraftBatch],
        verifier: FuzzyDraftVerifier,
        model_generate_one,
    ) -> None:
        self.grpc_client = grpc_client
        self.bid_queue = bid_queue
        self.verifier = verifier
        self.model_generate_one = model_generate_one

    async def cooperative_generate(self, prompt_tokens: list[str], request_id: str) -> AsyncIterator[str]:
        sequence_id = 0
        tokens = list(prompt_tokens)

        while not tokens or tokens[-1] != EOS_TOKEN:
            await self.grpc_client.broadcast_work(
                request_id=request_id,
                context_window=tokens[-100:],
                sequence_id=sequence_id,
                min_draft_len=5,
            )

            try:
                draft_batch = await asyncio.wait_for(self.bid_queue.get(), timeout=0.05)
                if draft_batch.request_id != request_id:
                    continue
                if draft_batch.sequence_id != sequence_id:
                    # Drop stale or future bids.
                    continue

                new_tokens = await self.verifier.verify_draft_batch(tokens, draft_batch.tokens)
                for tok in new_tokens:
                    tokens.append(tok)
                    sequence_id += 1
                    yield tok
                continue
            except asyncio.TimeoutError:
                pass

            one_token = await self.model_generate_one(tokens)
            tokens.append(one_token)
            sequence_id += 1
            yield one_token
