from __future__ import annotations

from collections.abc import AsyncIterator

from .transport import RouterClientConfig, ShardRouterClient


class _PromptEncoder:
    """Small encoder abstraction: use HF tokenizer if available, else UTF-8 byte ids."""

    def __init__(self, tokenizer_name: str | None = None) -> None:
        self._hf = None
        if tokenizer_name:
            try:
                from transformers import AutoTokenizer  # type: ignore

                self._hf = AutoTokenizer.from_pretrained(tokenizer_name)
            except Exception as exc:  # pragma: no cover - optional dependency path
                raise RuntimeError(
                    "Failed to load Hugging Face tokenizer. Install shard-client[hf] or omit tokenizer_name."
                ) from exc

    def encode(self, prompt: str) -> list[int]:
        if self._hf is not None:
            return list(self._hf.encode(prompt, add_special_tokens=True))
        return list(prompt.encode("utf-8"))


class ShardDistributedModel:
    """Drop-in style model wrapper that routes generation through local Shard sidecar.

    This intentionally resembles a subset of ``AutoModelForCausalLM`` ergonomics:
    - ``from_pretrained(...)`` constructor
    - ``generate(...)`` for full text output
    - ``stream_generate(...)`` for async token streaming
    """

    def __init__(
        self,
        model_name: str,
        *,
        router_config: RouterClientConfig | None = None,
        tokenizer_name: str | None = None,
        transport: str = "http_poll",
    ) -> None:
        self.model_name = model_name
        self.transport = transport
        self.router = ShardRouterClient(router_config)
        self.encoder = _PromptEncoder(tokenizer_name=tokenizer_name)

    @classmethod
    def from_pretrained(
        cls,
        model_name: str,
        *,
        router_url: str = "http://127.0.0.1:9091",
        websocket_url: str = "ws://127.0.0.1:9091/ws/generate",
        tokenizer_name: str | None = None,
        transport: str = "http_poll",
    ) -> "ShardDistributedModel":
        config = RouterClientConfig(http_base_url=router_url, websocket_url=websocket_url)
        return cls(
            model_name,
            router_config=config,
            tokenizer_name=tokenizer_name,
            transport=transport,
        )

    async def stream_generate(self, prompt: str, *, max_new_tokens: int = 128) -> AsyncIterator[str]:
        encoded = self.encoder.encode(prompt)
        if self.transport == "websocket":
            async for token in self.router.stream_tokens_websocket(
                prompt=prompt,
                encoded_prompt=encoded,
                max_new_tokens=max_new_tokens,
                model=self.model_name,
            ):
                yield token
            return

        async for token in self.router.stream_tokens_http_poll(
            prompt=prompt,
            encoded_prompt=encoded,
            max_new_tokens=max_new_tokens,
            model=self.model_name,
        ):
            yield token

    async def generate(self, prompt: str, *, max_new_tokens: int = 128) -> str:
        chunks: list[str] = []
        async for token in self.stream_generate(prompt, max_new_tokens=max_new_tokens):
            chunks.append(token)
        return "".join(chunks)

    async def aclose(self) -> None:
        await self.router.close()
