"""Synchronous Shard client using httpx."""

from __future__ import annotations

import json
import time
from typing import Generator, Iterator

import httpx

from shard_sdk.exceptions import (
    ShardAPIError,
    ShardAuthError,
    ShardConnectionError,
    ShardTimeoutError,
)
from shard_sdk.models import (
    ChatCompletionRequest,
    ChatCompletionResponse,
    ChatMessage,
    StreamChunk,
)


_DEFAULT_BASE_URL = "http://localhost:8080"
_DEFAULT_TIMEOUT = 30.0
_MAX_RETRIES = 3
_RETRY_BACKOFF_BASE = 0.5


class ShardClient:
    """Synchronous client for the Shard distributed inference API.

    OpenAI-compatible interface — can be used as a drop-in replacement.

    Usage:
        client = ShardClient(api_key="sk_...")
        response = client.chat("Hello, world!")
        print(response.choices[0].message.content)
    """

    def __init__(
        self,
        *,
        api_key: str | None = None,
        base_url: str = _DEFAULT_BASE_URL,
        timeout: float = _DEFAULT_TIMEOUT,
        max_retries: int = _MAX_RETRIES,
    ):
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self.max_retries = max_retries

        headers: dict[str, str] = {"Content-Type": "application/json"}
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        self._client = httpx.Client(
            base_url=self.base_url,
            headers=headers,
            timeout=timeout,
        )

    def close(self) -> None:
        """Close the underlying HTTP client."""
        self._client.close()

    def __enter__(self) -> "ShardClient":
        return self

    def __exit__(self, *args: object) -> None:
        self.close()

    # ─── Chat Completions API ────────────────────────────────────────

    def chat(
        self,
        messages: str | list[dict[str, str]] | list[ChatMessage],
        *,
        model: str = "default",
        stream: bool = False,
        temperature: float = 0.7,
        max_tokens: int | None = None,
        sensitive: bool = False,
    ) -> ChatCompletionResponse | Generator[StreamChunk, None, None]:
        """Send a chat completion request.

        Args:
            messages: A string (auto-wrapped as user message), list of dicts,
                      or list of ChatMessage objects.
            model: Model identifier.
            stream: If True, returns a generator yielding StreamChunk objects.
            temperature: Sampling temperature.
            max_tokens: Maximum tokens to generate.
            sensitive: If True, routes via private mesh (X-Shard-Route: private).

        Returns:
            ChatCompletionResponse (non-streaming) or Generator[StreamChunk] (streaming).
        """
        # Normalize messages
        if isinstance(messages, str):
            chat_messages = [ChatMessage(role="user", content=messages)]
        elif messages and isinstance(messages[0], dict):
            chat_messages = [ChatMessage(**m) for m in messages]  # type: ignore
        else:
            chat_messages = list(messages)  # type: ignore

        request = ChatCompletionRequest(
            model=model,
            messages=chat_messages,
            stream=stream,
            temperature=temperature,
            max_tokens=max_tokens,
            sensitive=sensitive,
        )

        if stream:
            return self._stream_chat(request)
        else:
            return self._sync_chat(request)

    def _sync_chat(self, request: ChatCompletionRequest) -> ChatCompletionResponse:
        """Non-streaming chat completion with retry."""
        body = request.model_dump(exclude_none=True)
        headers = self._extra_headers(request)

        for attempt in range(self.max_retries):
            try:
                response = self._client.post(
                    "/v1/chat/completions",
                    json=body,
                    headers=headers,
                )
                if response.status_code == 401:
                    raise ShardAuthError("Invalid or missing API key")
                if response.status_code == 429:
                    # Rate limited — retry with backoff
                    self._backoff(attempt)
                    continue
                if response.status_code >= 500:
                    self._backoff(attempt)
                    continue
                if response.status_code >= 400:
                    raise ShardAPIError(
                        f"API error: {response.text}",
                        status_code=response.status_code,
                        response_body=response.text,
                    )
                return ChatCompletionResponse.model_validate(response.json())
            except httpx.TimeoutException:
                if attempt == self.max_retries - 1:
                    raise ShardTimeoutError(
                        f"Request timed out after {self.timeout}s ({self.max_retries} attempts)"
                    )
                self._backoff(attempt)
            except httpx.ConnectError:
                raise ShardConnectionError(
                    f"Cannot connect to Shard daemon at {self.base_url}"
                )

        raise ShardAPIError("Max retries exceeded")

    def _stream_chat(
        self, request: ChatCompletionRequest
    ) -> Generator[StreamChunk, None, None]:
        """SSE streaming chat completion."""
        body = request.model_dump(exclude_none=True)
        headers = self._extra_headers(request)

        try:
            with self._client.stream(
                "POST", "/v1/chat/completions", json=body, headers=headers
            ) as response:
                if response.status_code == 401:
                    raise ShardAuthError("Invalid or missing API key")
                if response.status_code >= 400:
                    response.read()
                    raise ShardAPIError(
                        f"API error: {response.text}",
                        status_code=response.status_code,
                        response_body=response.text,
                    )
                for line in response.iter_lines():
                    if not line:
                        continue
                    if line.startswith("data: "):
                        data = line[6:]
                        if data.strip() == "[DONE]":
                            return
                        try:
                            chunk = StreamChunk.model_validate(json.loads(data))
                            yield chunk
                        except (json.JSONDecodeError, Exception):
                            continue
        except httpx.TimeoutException:
            raise ShardTimeoutError(f"Stream timed out after {self.timeout}s")
        except httpx.ConnectError:
            raise ShardConnectionError(
                f"Cannot connect to Shard daemon at {self.base_url}"
            )

    def _extra_headers(self, request: ChatCompletionRequest) -> dict[str, str]:
        """Build extra headers (e.g., privacy routing)."""
        headers: dict[str, str] = {}
        if request.sensitive:
            if not self.api_key:
                import warnings
                warnings.warn(
                    "sensitive=True but no API key set — request may be rejected",
                    stacklevel=3,
                )
            headers["X-Shard-Route"] = "private"
        return headers

    @staticmethod
    def _backoff(attempt: int) -> None:
        """Exponential backoff with jitter."""
        delay = _RETRY_BACKOFF_BASE * (2**attempt)
        time.sleep(delay)
