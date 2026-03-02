import asyncio
from shard_sdk import AsyncShardClient

async def test_async_client():
    try:
        async with AsyncShardClient(api_key="test") as client:
            print("Async client initialized")
            if hasattr(client, "chat") and hasattr(client.chat, "completions") and hasattr(client.chat.completions, "create"):
                print("Async interface check passed: client.chat.completions.create exists")
            else:
                print("Async interface check failed")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    asyncio.run(test_async_client())
