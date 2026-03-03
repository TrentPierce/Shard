from __future__ import annotations

import json
from collections.abc import Iterator
from typing import TYPE_CHECKING

from shard.models.chat import ChatRequest, ChatResponse, ChatStreamChunk

if TYPE_CHECKING:
    from shard.client import Client


class ChatCompletionsResource:
    def __init__(self, client: "Client") -> None:
        self._client = client

    def create(
        self,
        model: str,
        messages: list[dict],
        stream: bool = False,
        max_tokens: int | None = None,
    ) -> ChatResponse | Iterator[ChatStreamChunk]:
        """Create a chat completion.

        Args:
            model: Model identifier.
            messages: OpenAI-style message list.
            stream: Whether to return SSE chunks.
            max_tokens: Optional max generated tokens.

        Returns:
            ChatResponse for non-streaming calls, or iterator of ChatStreamChunk for streaming.
        """
        payload = ChatRequest(model=model, messages=messages, stream=stream, max_tokens=max_tokens)
        if stream:
            return self._stream_chunks(payload.model_dump(exclude_none=True))
        response = self._client._request("POST", "/v1/chat/completions", json=payload.model_dump(exclude_none=True))
        return ChatResponse.model_validate(response.json())

    def _stream_chunks(self, payload: dict) -> Iterator[ChatStreamChunk]:
        with self._client._http.stream("POST", "/v1/chat/completions", json=payload) as response:
            self._client._raise_for_status(response)
            for line in response.iter_lines():
                if not line:
                    continue
                if not line.startswith("data:"):
                    continue
                raw = line[5:].strip()
                if raw == "[DONE]":
                    break
                yield ChatStreamChunk.model_validate(json.loads(raw))


class ChatResource:
    def __init__(self, client: "Client") -> None:
        self._client = client
        self.completions = ChatCompletionsResource(client)
