from shard_sdk import ShardClient

def test_sync_client():
    # We don't need a real server to just check the interface
    try:
        client = ShardClient(api_key="test")
        print("Synchronous client initialized")
        # Just check if the attributes exist
        if hasattr(client, "chat") and hasattr(client.chat, "completions") and hasattr(client.chat.completions, "create"):
            print("Sync interface check passed: client.chat.completions.create exists")
        else:
            print("Sync interface check failed")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    test_sync_client()
