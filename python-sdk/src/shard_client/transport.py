from __future__ import annotations

import asyncio
import json
import uuid
from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Any

import httpx
import websockets


@dataclass(slots=True)
class RouterClientConfig:
    """Config for talking to the local Rust sidecar."""

    http_base_url: str = "http://127.0.0.1:9091"
    websocket_url: str = "ws://127.0.0.1:9091/ws/generate"
    broadcast_endpoint: str = "/broadcast-work"
    pop_result_endpoint: str = "/pop-result"
    request_timeout_s: float = 5.0
    poll_interval_s: float = 0.03


class SidecarProtocolError(RuntimeError):
    """Raised when sidecar responses are malformed or unsupported."""


class ShardRouterClient:
    """Minimal transport client for HTTP polling or websocket token streams."""

    def __init__(self, config: RouterClientConfig | None = None) -> None:
        self.config = config or RouterClientConfig()
        self._http = httpx.AsyncClient(
            base_url=self.config.http_base_url,
            timeout=self.config.request_timeout_s,
        )

    async def close(self) -> None:
        await self._http.aclose()

    async def stream_tokens_http_poll(
        self,
        *,
        prompt: str,
        encoded_prompt: list[int],
        max_new_tokens: int,
        model: str,
    ) -> AsyncIterator[str]:
        request_id = f"sdk-{uuid.uuid4()}"
        payload = {
            "request_id": request_id,
            "prompt_context": prompt,
            "input_token_ids": encoded_prompt,
            "min_tokens": 1,
            "max_new_tokens": max_new_tokens,
            "model": model,
            "sdk": "shard-client/0.1.0",
        }
        resp = await self._http.post(self.config.broadcast_endpoint, json=payload)
        resp.raise_for_status()

        emitted = 0
        while emitted < max_new_tokens:
            poll = await self._http.get(self.config.pop_result_endpoint, params={"request_id": request_id})
            poll.raise_for_status()
            data = poll.json()
            result = data.get("result") if isinstance(data, dict) else None
            token = _extract_token(result)
            if token is None:
                if _is_terminal(result):
                    break
                await asyncio.sleep(self.config.poll_interval_s)
                continue

            emitted += 1
            yield token

    async def stream_tokens_websocket(
        self,
        *,
        prompt: str,
        encoded_prompt: list[int],
        max_new_tokens: int,
        model: str,
    ) -> AsyncIterator[str]:
        request = {
            "request_id": f"sdk-{uuid.uuid4()}",
            "prompt": prompt,
            "input_token_ids": encoded_prompt,
            "max_new_tokens": max_new_tokens,
            "model": model,
            "stream": True,
            "sdk": "shard-client/0.1.0",
        }

        async with websockets.connect(self.config.websocket_url) as ws:
            await ws.send(json.dumps(request))
            async for raw in ws:
                message = json.loads(raw)
                if message.get("event") in {"done", "eos", "completed"}:
                    break
                token = _extract_token(message)
                if token is not None:
                    yield token


def _extract_token(payload: Any) -> str | None:
    if not isinstance(payload, dict):
        return None
    for key in ("token", "text", "delta", "draft_text"):
        value = payload.get(key)
        if isinstance(value, str) and value:
            return value
    return None


def _is_terminal(payload: Any) -> bool:
    if not isinstance(payload, dict):
        return False
    return bool(payload.get("done") or payload.get("eos") or payload.get("completed"))
