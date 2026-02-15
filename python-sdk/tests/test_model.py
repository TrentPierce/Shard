import asyncio

from shard_client.model import ShardDistributedModel


class _StubRouter:
    async def stream_tokens_http_poll(self, **kwargs):
        for token in ["A", "B", "C"]:
            yield token

    async def close(self):
        return None


async def _collect_text() -> str:
    model = ShardDistributedModel("test-model")
    model.router = _StubRouter()  # type: ignore[assignment]
    return await model.generate("hello", max_new_tokens=3)


def test_generate_joins_streamed_tokens() -> None:
    output = asyncio.run(_collect_text())
    assert output == "ABC"
