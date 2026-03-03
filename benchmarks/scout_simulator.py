from __future__ import annotations

import asyncio
import hashlib
import random
import string
import time
from dataclasses import dataclass
from typing import Any

import httpx


@dataclass(frozen=True)
class ScoutIdentity:
    scout_id: str


def _leading_zero_bits(digest: bytes) -> int:
    total = 0
    for byte in digest:
        if byte == 0:
            total += 8
            continue
        total += 8 - byte.bit_length()
        break
    return total


def _solve_pow(challenge_hex: str, difficulty: int) -> tuple[int, str]:
    challenge = bytes.fromhex(challenge_hex)
    nonce = 0
    while True:
        digest = hashlib.sha256(challenge + nonce.to_bytes(8, byteorder="little", signed=False)).digest()
        if _leading_zero_bits(digest) >= difficulty:
            return nonce, digest.hex()
        nonce += 1


class ScoutSimulator:
    def __init__(
        self,
        base_url: str,
        scout_count: int,
        duration_seconds: int,
        collector: Any,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.scout_count = scout_count
        self.duration_seconds = duration_seconds
        self.collector = collector
        self._end_time = 0.0
        self._rng = random.Random(3326)

    async def _register_pow(self, client: httpx.AsyncClient, identity: ScoutIdentity) -> bool:
        try:
            challenge_response = await client.get(
                f"{self.base_url}/v1/pow/challenge",
                params={
                    "peer_id": identity.scout_id,
                    "hardware_concurrency": 8,
                    "is_mobile": False,
                },
            )
            challenge_response.raise_for_status()
            payload = challenge_response.json()
            challenge = payload.get("challenge") or {}
            challenge_hex = challenge.get("challenge_bytes_hex")
            difficulty = int(challenge.get("difficulty", 16))
            if not challenge_hex:
                return False

            nonce, hash_hex = await asyncio.to_thread(_solve_pow, challenge_hex, difficulty)
            verify_response = await client.post(
                f"{self.base_url}/v1/pow/verify",
                json={"peer_id": identity.scout_id, "nonce": nonce, "hash_hex": hash_hex},
            )
            verify_response.raise_for_status()
            return bool(verify_response.json().get("ok", False))
        except Exception:
            return False

    def _make_identity(self, index: int) -> ScoutIdentity:
        suffix = "".join(self._rng.choices(string.ascii_lowercase + string.digits, k=6))
        return ScoutIdentity(scout_id=f"bench_scout_{index}_{suffix}")

    def _build_draft_logits(self, vocab_size: int = 64, token_count: int = 8) -> list[list[float]]:
        return [
            [self._rng.uniform(-4.0, 4.0) for _ in range(vocab_size)]
            for _ in range(token_count)
        ]

    async def _run_one_scout(self, client: httpx.AsyncClient, identity: ScoutIdentity) -> None:
        pow_ok = await self._register_pow(client, identity)
        if not pow_ok:
            self.collector.record(
                draft_accepted=False,
                latency_ms=0.0,
                tokens_in_draft=0,
                error="pow_verification_failed",
                request_bytes=0,
                response_bytes=0,
            )
            return

        while time.monotonic() < self._end_time:
            start = time.monotonic()
            tokens_in_draft = 0
            req_bytes = 0
            resp_bytes = 0
            try:
                work_resp = await client.get(
                    f"{self.base_url}/v1/scout/work",
                    params={"scout_id": identity.scout_id},
                )
                resp_bytes += len(work_resp.content)
                work_resp.raise_for_status()
                work_json = work_resp.json()
                work = work_json.get("work")
                if not work:
                    await asyncio.sleep(0.05)
                    continue

                work_id = str(work.get("request_id") or work.get("work_id") or "")
                prompt = str(work.get("prompt") or work.get("prompt_context") or "")
                if not work_id:
                    self.collector.record(
                        draft_accepted=False,
                        latency_ms=(time.monotonic() - start) * 1000.0,
                        tokens_in_draft=0,
                        error="missing_work_id",
                        request_bytes=req_bytes,
                        response_bytes=resp_bytes,
                    )
                    continue

                fake_logits = self._build_draft_logits()
                draft_tokens = [int(self._rng.randint(1, 4096)) for _ in range(len(fake_logits))]
                tokens_in_draft = len(draft_tokens)
                submission = {
                    "work_id": work_id,
                    "scout_id": identity.scout_id,
                    "draft_text": " ".join(f"tok{token}" for token in draft_tokens),
                    "prompt_context": prompt,
                    "draft_tokens": draft_tokens,
                    "draft_logits": fake_logits,
                    "timestamp": time.time(),
                    "scout_mode": "webgpu",
                }
                body = str(submission).encode("utf-8")
                req_bytes += len(body)

                draft_resp = await client.post(f"{self.base_url}/v1/scout/draft", json=submission)
                resp_bytes += len(draft_resp.content)
                draft_resp.raise_for_status()
                draft_json = draft_resp.json()

                accepted = bool(draft_json.get("ok", False))
                detail = str(draft_json.get("detail") or "")
                error = None if accepted else (detail or "draft_rejected")

                self.collector.record(
                    draft_accepted=accepted,
                    latency_ms=(time.monotonic() - start) * 1000.0,
                    tokens_in_draft=tokens_in_draft,
                    error=error,
                    request_bytes=req_bytes,
                    response_bytes=resp_bytes,
                )
            except Exception as exc:
                self.collector.record(
                    draft_accepted=False,
                    latency_ms=(time.monotonic() - start) * 1000.0,
                    tokens_in_draft=tokens_in_draft,
                    error=str(exc),
                    request_bytes=req_bytes,
                    response_bytes=resp_bytes,
                )

    async def run(self) -> None:
        self._end_time = time.monotonic() + float(self.duration_seconds)
        limits = httpx.Limits(
            max_keepalive_connections=max(50, self.scout_count * 2),
            max_connections=max(200, self.scout_count * 4),
        )
        timeout = httpx.Timeout(connect=10.0, read=30.0, write=30.0, pool=30.0)

        async with httpx.AsyncClient(limits=limits, timeout=timeout) as client:
            tasks = [
                asyncio.create_task(self._run_one_scout(client, self._make_identity(i)))
                for i in range(self.scout_count)
            ]
            await asyncio.sleep(self.duration_seconds)
            await asyncio.gather(*tasks, return_exceptions=True)
