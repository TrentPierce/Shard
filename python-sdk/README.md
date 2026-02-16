# shard-client (scaffold)

Initial scaffolding for a drop-in Python SDK that routes inference through the local Shard Rust sidecar.

## What this does

- Exposes `ShardDistributedModel.from_pretrained(...)` to mimic `AutoModelForCausalLM` style construction.
- Encodes prompts into token IDs (Hugging Face tokenizer if configured, UTF-8 byte IDs fallback).
- Sends prompt activations (`input_token_ids`) to the local sidecar over HTTP.
- Streams returning tokens asynchronously via either:
  - HTTP polling (`/broadcast-work` + `/pop-result`), or
  - WebSocket (`/ws/generate`).

## Install (local scaffold)

```bash
cd python-sdk
pip install -e .
```

Optional Hugging Face tokenizer support:

```bash
pip install -e .[hf]
```

## Example

```python
import asyncio
from shard_client import ShardDistributedModel


async def main() -> None:
    model = ShardDistributedModel.from_pretrained(
        "shard/network-default",
        router_url="http://127.0.0.1:9091",
        transport="http_poll",  # or "websocket"
    )

    async for token in model.stream_generate("Explain speculative decoding in one sentence.", max_new_tokens=64):
        print(token, end="", flush=True)

    await model.aclose()


asyncio.run(main())
```

## Sidecar API assumptions (to align with your local implementation)

### HTTP polling mode

`POST /broadcast-work`

```json
{
  "request_id": "sdk-...",
  "prompt_context": "...",
  "input_token_ids": [1, 2, 3],
  "min_tokens": 1,
  "max_new_tokens": 128,
  "model": "shard/network-default"
}
```

`GET /pop-result?request_id=sdk-...`

```json
{
  "result": {
    "token": " next"
  }
}
```

### WebSocket mode

`ws://127.0.0.1:9091/ws/generate`

```json
{
  "request_id": "sdk-...",
  "prompt": "...",
  "input_token_ids": [1, 2, 3],
  "max_new_tokens": 128,
  "stream": true
}
```

Token message example:

```json
{ "token": " next" }
```

Done message example:

```json
{ "event": "done" }
```

---

If your sidecar endpoints differ, we can wire exact paths/payloads in the next pass.
