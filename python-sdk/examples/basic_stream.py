import asyncio

from shard_client import ShardDistributedModel


async def main() -> None:
    model = ShardDistributedModel.from_pretrained("shard/network-default")
    try:
        async for token in model.stream_generate("Hello from Shard!", max_new_tokens=32):
            print(token, end="", flush=True)
        print()
    finally:
        await model.aclose()


if __name__ == "__main__":
    asyncio.run(main())
