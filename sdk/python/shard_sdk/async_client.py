"""Async Shard client using httpx.AsyncClient."""

from __future__ import annotations

import asyncio
import json
from typing import AsyncGenerator

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


class AsyncShardClient:
    """Async client for the Shard distributed inference API.

    Usage:
        async with AsyncShardClient(api_key="sk_...") as client:
            response = await client.chat.completions.create(
                messages=[{"role": "user", "content": "Hello!"}],
                model="default"
            )
            print(response.choices[0].message.content)

            # Streaming
            stream = await client.chat.completions.create(
                messages="Tell me a story", 
                stream=True
            )
            async for chunk in stream:
                print(chunk.choices[0].delta.content, end="")
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

        self._client = httpx.AsyncClient(
            base_url=self.base_url,
            headers=headers,
            timeout=timeout,
        )
        
        # Resources
        self.chat = AsyncChat(self)

    async def close(self) -> None:
        """Close the underlying HTTP client."""
        await self._client.aclose()

    async def __aenter__(self) -> "AsyncShardClient":
        return self

    async def __aexit__(self, *args: object) -> None:
        await self.close()


class AsyncChat:
    def __init__(self, client: AsyncShardClient):
        self.completions = AsyncCompletions(client)


class AsyncCompletions:
    def __init__(self, client: AsyncShardClient):
        self._client = client

    async def create(
        self,
        *,
        messages: str | list[dict[str, str]] | list[ChatMessage],
        model: str = "default",
        stream: bool = False,
        temperature: float = 0.7,
        max_tokens: int | None = None,
        top_p: float = 1.0,
        sensitive: bool = False,
        **kwargs: object,
    ) -> ChatCompletionResponse | AsyncGenerator[StreamChunk, None]:
        """Send an async chat completion request.

        Args:
            messages: A string, list of dicts, or list of ChatMessage objects.
            model: Model identifier.
            stream: If True, returns an async generator of StreamChunk objects.
            temperature: Sampling temperature.
            max_tokens: Maximum tokens to generate.
            top_p: Nucleus sampling parameter.
            sensitive: If True, routes via private mesh.
            **kwargs: Additional parameters (ignored).

        Returns:
            ChatCompletionResponse (non-streaming) or AsyncGenerator[StreamChunk].
        """
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
            top_p=top_p,
            sensitive=sensitive,
        )

        if stream:
            return self._client._stream_chat(request)
        else:
            return await self._client._sync_chat(request)

    async def _sync_chat(
        self, request: ChatCompletionRequest
    ) -> ChatCompletionResponse:
        body = request.model_dump(exclude_none=True)
        headers = self._extra_headers(request)

        for attempt in range(self.max_retries):
            try:
                response = await self._client.post(
                    "/v1/chat/completions",
                    json=body,
                    headers=headers,
                )
                if response.status_code == 401:
                    raise ShardAuthError("Invalid or missing API key")
                if response.status_code == 429:
                    await self._backoff(attempt)
                    continue
                if response.status_code >= 500:
                    await self._backoff(attempt)
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
                        f"Request timed out after {self.timeout}s"
                    )
                await self._backoff(attempt)
            except httpx.ConnectError:
                raise ShardConnectionError(
                    f"Cannot connect to Shard daemon at {self.base_url}"
                )

        raise ShardAPIError("Max retries exceeded")

    async def _stream_chat(
        self, request: ChatCompletionRequest
    ) -> AsyncGenerator[StreamChunk, None]:
        body = request.model_dump(exclude_none=True)
        headers = self._extra_headers(request)

        try:
            async with self._client.stream(
                "POST", "/v1/chat/completions", json=body, headers=headers
            ) as response:
                if response.status_code == 401:
                    raise ShardAuthError("Invalid or missing API key")
                if response.status_code >= 400:
                    await response.aread()
                    raise ShardAPIError(
                        f"API error: {response.text}",
                        status_code=response.status_code,
                        response_body=response.text,
                    )
                async for line in response.aiter_lines():
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
        headers: dict[str, str] = {}
        if request.sensitive:
            if not self.api_key:
                import warnings
                warnings.warn(
                    "sensitive=True but no API key set",
                    stacklevel=3,
                )
            headers["X-Shard-Route"] = "private"
        return headers

    @staticmethod
    async def _backoff(attempt: int) -> None:
        delay = _RETRY_BACKOFF_BASE * (2**attempt)
        await asyncio.sleep(delay)
