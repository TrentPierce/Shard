# Shard Python SDK

Python client for the Shard distributed inference network.

## Installation

```bash
pip install shard-inference
```

### Run a local Shard node (public mesh)

After installing, the `shard-daemon` binary is available on your `PATH` (or in
your virtualenv `Scripts/` directory on Windows). To join the public
`shardnetwork.live` mesh as a verifier node:

```bash
shard-daemon \
  --control-port 9091 \
  --tcp-port 4001 \
  --webrtc-port 9090 \
  --quic-port 9092 \
  --bootstrap-node /ip4/35.175.242.222/tcp/4001/p2p/12D3KooWConhJakwyGN72uZ1Jtxi3LFecN3cYKxEX3aNLDAo48by \
  --bootstrap-node /dns4/bootstrap-2.shardnetwork.live/tcp/4001/p2p/12D3KooWExamplePeerTwo \
  --bootstrap-node /dns4/bootstrap-3.shardnetwork.live/udp/9092/quic-v1/p2p/12D3KooWExamplePeerThree
```

Once running, visit `https://shardnetwork.live` and your node will appear in the
network view (it may take a few seconds after the first successful handshake).
For self-hosted clusters, replace the `--bootstrap-node` address with your own
seed peer or set `SHARD_DEFAULT_BOOTSTRAP` as described in `docs/ENVIRONMENT.md`.

Or install from source:

```bash
cd sdk/python
pip install -e .
```

## Quick Start

```python
from shard import Shard

# Connect to local daemon (default: http://localhost:9091)
client = Shard()

# Non-streaming request
response = client.chat.completions.create(
    model="shard-hybrid",
    messages=[{"role": "user", "content": "Explain quantum computing simply."}]
)
print(response.choices[0].message.content)

# Streaming request
stream = client.chat.completions.create(
    model="shard-hybrid",
    messages=[{"role": "user", "content": "Count to 5"}],
    stream=True
)
for chunk in stream:
    if chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)

client.close()
```

## OpenAI-Compatible Drop-In

The SDK is designed to be a drop-in replacement for the OpenAI client:

```python
# Instead of:
from openai import OpenAI
client = OpenAI(base_url="http://localhost:9091/v1", api_key="...")

# You can use:
from shard import Shard
client = Shard()  # Defaults to http://localhost:9091
```

All standard OpenAI parameters are supported:
- `messages` - Chat messages
- `model` - Model identifier
- `temperature` - Sampling temperature (0.0-2.0)
- `max_tokens` - Maximum tokens to generate
- `top_p` - Nucleus sampling parameter
- `stream` - Enable streaming responses

## Async Usage

```python
import asyncio
from shard import AsyncShard

async def main():
    async with AsyncShard() as client:
        response = await client.chat.completions.create(
            model="shard-hybrid",
            messages=[{"role": "user", "content": "Hello!"}]
        )
        print(response.choices[0].message.content)

asyncio.run(main())
```

## API Key & Authentication

```python
# With API key
client = Shard(api_key="sk-your-key-here")

# Connect to remote server
client = Shard(base_url="https://your-shard-server.com", api_key="sk-...")
```

## Private Mesh Routing

For sensitive requests that should bypass public bootstrap peers:

```python
response = client.chat.completions.create(
    model="shard-hybrid",
    messages=[...],
    sensitive=True  # Routes via X-Shard-Route: private
)
```

## Configuration

```python
client = Shard(
    api_key="sk-...",           # API key (optional)
    base_url="http://localhost:9091",  # Daemon URL
    timeout=30.0,               # Request timeout (seconds)
    max_retries=3,              # Retry attempts on failure
)
```

## Exceptions

The SDK provides typed exceptions:

- `ShardError` - Base exception
- `ShardAPIError` - API errors (4xx/5xx responses)
- `ShardAuthError` - Authentication failures (401)
- `ShardTimeoutError` - Request timeouts
- `ShardConnectionError` - Connection failures

```python
from shard import Shard, ShardAPIError

try:
    client = Shard()
    response = client.chat.completions.create(messages=[...])
except ShardAPIError as e:
    print(f"API Error: {e.status_code} - {e.response_body}")
except ShardTimeoutError:
    print("Request timed out")
```

## Requirements

- Python 3.9+
- httpx >= 0.25.0
- pydantic >= 2.0

For async support:
- aiohttp >= 3.9.0

## License

MIT
