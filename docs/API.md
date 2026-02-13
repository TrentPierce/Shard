# Shard API Documentation

Version: 0.4.0

This document provides comprehensive documentation for the Shard Oracle API, which follows the OpenAI API specification for compatibility with existing clients.

---

## Table of Contents

- [Overview](#overview)
- [Authentication](#authentication)
- [Base URL](#base-url)
- [Endpoints](#endpoints)
  - [Chat Completions](#chat-completions)
  - [Models](#models)
  - [System Endpoints](#system-endpoints)
  - [Health & Monitoring](#health--monitoring)
- [Error Codes](#error-codes)
- [Rate Limiting](#rate-limiting)
- [Streaming Responses](#streaming-responses)

---

## Overview

The Shard Oracle API provides an OpenAI-compatible interface for interacting with the distributed inference network. The API supports:

- Chat completions with streaming
- Hybrid inference (Oracle verification + Scout generation)
- Network topology discovery
- System health monitoring

### API Versioning

The API is versioned using URL paths (e.g., `/v1/chat/completions`). Major version changes will be announced in the changelog.

---

## Authentication

Authentication is optional and controlled via the `SHARD_API_KEYS` environment variable.

### Enabling Authentication

Set one or more API keys:

```bash
export SHARD_API_KEYS="key1,key2,key3"
```

### Making Authenticated Requests

Include the API key using either the `Authorization` header (Bearer token) or `X-API-Key` header:

```bash
# Using Authorization header
curl -H "Authorization: Bearer your-api-key" \
  https://api.shard.network/v1/chat/completions

# Using X-API-Key header
curl -H "X-API-Key: your-api-key" \
  https://api.shard.network/v1/chat/completions
```

### Response

Unauthenticated requests (when authentication is required):

```json
{
  "detail": "Missing or invalid API key"
}
```

---

## Base URL

The base URL depends on your deployment:

| Environment | Base URL |
|-------------|----------|
| Local Development | `http://localhost:8000` |
| Production | `https://api.shard.network` |

All endpoints are relative to the base URL.

---

## Endpoints

### Chat Completions

Create a chat completion using the distributed inference network.

#### `POST /v1/chat/completions`

Creates a model response for the given chat conversation. Supports both streaming and non-streaming modes.

##### Request Headers

| Header | Type | Required | Description |
|--------|------|----------|-------------|
| `Content-Type` | string | Yes | Must be `application/json` |
| `Authorization` | string | If auth enabled | Bearer token |
| `X-API-Key` | string | If auth enabled | API key |

##### Request Body

```json
{
  "model": "shard-hybrid",
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful assistant."
    },
    {
      "role": "user",
      "content": "Explain quantum computing in simple terms."
    }
  ],
  "temperature": 0.7,
  "max_tokens": 256,
  "stream": false
}
```

##### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `model` | string | Yes | - | Model ID to use. Currently only `shard-hybrid` is supported. |
| `messages` | array | Yes | - | Array of message objects |
| `temperature` | number | No | 0.7 | Sampling temperature (0.0 - 2.0) |
| `max_tokens` | number | No | 256 | Maximum tokens to generate (1 - 2048) |
| `stream` | boolean | No | false | Enable streaming responses |

##### Message Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `role` | string | Yes | One of: `system`, `user`, `assistant` |
| `content` | string | Yes | Message content (1 - 8000 characters) |

##### Non-Streaming Response

```json
{
  "id": "chatcmpl-abc123def456",
  "object": "chat.completion",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Quantum computing uses quantum bits or qubits..."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 15,
    "completion_tokens": 42,
    "total_tokens": 57
  }
}
```

##### Streaming Response

When `stream: true`, the API returns Server-Sent Events (SSE):

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1234567890,"model":"shard-hybrid","choices":[{"index":0,"delta":{"content":"Quantum "},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1234567890,"model":"shard-hybrid","choices":[{"index":0,"delta":{"content":"computing "},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1234567890,"model":"shard-hybrid","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

##### Example: cURL (Non-Streaming)

```bash
curl https://api.shard.network/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '{
    "model": "shard-hybrid",
    "messages": [{"role": "user", "content": "Hello!"}],
    "max_tokens": 100
  }'
```

##### Example: cURL (Streaming)

```bash
curl https://api.shard.network/v1/chat/completions \
  -H "Content-Type: application/json" \
  -N \
  -d '{
    "model": "shard-hybrid",
    "messages": [{"role": "user", "content": "Tell me a story"}],
    "stream": true
  }'
```

##### Example: Python

```python
import requests

response = requests.post(
    "https://api.shard.network/v1/chat/completions",
    headers={
        "Authorization": "Bearer your-api-key",
        "Content-Type": "application/json",
    },
    json={
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": "Hello, Shard!"}],
        "max_tokens": 100,
    },
)

result = response.json()
print(result["choices"][0]["message"]["content"])
```

##### Example: Python (Streaming)

```python
import requests

response = requests.post(
    "https://api.shard.network/v1/chat/completions",
    headers={
        "Content-Type": "application/json",
    },
    json={
        "model": "shard-hybrid",
        "messages": [{"role": "user", "content": "Tell me a story"}],
        "stream": True,
    },
    stream=True,
)

for line in response.iter_lines():
    if line.startswith(b"data: "):
        data = line[6:]
        if data == b"[DONE]":
            break
        chunk = json.loads(data)
        content = chunk["choices"][0]["delta"].get("content", "")
        print(content, end="", flush=True)
```

##### Example: JavaScript

```javascript
const response = await fetch('https://api.shard.network/v1/chat/completions', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': 'Bearer your-api-key',
  },
  body: JSON.stringify({
    model: 'shard-hybrid',
    messages: [{ role: 'user', content: 'Hello, Shard!' }],
    max_tokens: 100,
  }),
});

const data = await response.json();
console.log(data.choices[0].message.content);
```

---

### Models

List available models.

#### `GET /v1/models`

Returns a list of available models for use with the API.

##### Response

```json
{
  "object": "list",
  "data": [
    {
      "id": "shard-hybrid",
      "object": "model",
      "owned_by": "shard-network",
      "permission": []
    }
  ]
}
```

##### Example: cURL

```bash
curl https://api.shard.network/v1/models
```

---

### System Endpoints

System endpoints provide information about the network topology and peer status.

#### `GET /v1/system/topology`

Retrieve network topology information for browser auto-discovery.

##### Response

```json
{
  "status": "ok",
  "source": "rust-sidecar",
  "oracle_peer_id": "QmXw...",
  "oracle_webrtc_multiaddr": "/ip4/192.168.1.10/udp/9090/webrtc-direct/p2p/QmXw...",
  "oracle_ws_multiaddr": "/ip4/192.168.1.10/tcp/4101/ws/p2p/QmXw...",
  "listen_addrs": [
    "/ip4/192.168.1.10/tcp/4001",
    "/ip4/192.168.1.10/tcp/4101/ws"
  ],
  "known_peer_count": 5
}
```

##### Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | `ok` or `degraded` |
| `source` | string | Data source (`rust-sidecar` or `fallback`) |
| `oracle_peer_id` | string | libp2p peer ID of the Oracle |
| `oracle_webrtc_multiaddr` | string | WebRTC multiaddr (Linux/Mac only) |
| `oracle_ws_multiaddr` | string | WebSocket multiaddr |
| `listen_addrs` | array | All listening addresses |
| `known_peer_count` | number | Number of known peers |

---

#### `GET /v1/system/peers`

Retrieve information about connected peers.

##### Response

```json
{
  "peers": [
    {
      "peer_id": "QmYw...",
      "connected_at": 1704067200000,
      "last_seen_at": 1704067260000,
      "addrs": [
        "/ip4/192.168.1.20/tcp/4001"
      ],
      "verified": true,
      "handshake_failures": 0
    }
  ],
  "count": 1
}
```

##### Peer Object

| Field | Type | Description |
|-------|------|-------------|
| `peer_id` | string | libp2p peer ID |
| `connected_at` | number | Connection timestamp (ms since epoch) |
| `last_seen_at` | number | Last activity timestamp (ms since epoch) |
| `addrs` | array | Peer addresses |
| `verified` | boolean | Whether peer has completed handshake |
| `handshake_failures` | number | Number of failed handshake attempts |

---

### Health & Monitoring

#### `GET /health`

Check the health status of the API and its dependencies.

##### Response

```json
{
  "status": "ok",
  "idle": false,
  "accepting_swarm_jobs": false,
  "rust_sidecar": "connected",
  "rust_url": "http://127.0.0.1:9091",
  "bitnet_loaded": true,
  "cors_origins": ["http://localhost:3000", "http://127.0.0.1:3000"]
}
```

##### Fields

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Overall status (`ok` or `error`) |
| `idle` | boolean | Whether the node is idle |
| `accepting_swarm_jobs` | boolean | Whether accepting distributed work |
| `rust_sidecar` | string | Connection status to Rust daemon |
| `rust_url` | string | Rust daemon URL |
| `bitnet_loaded` | boolean | Whether BitNet runtime is loaded |
| `cors_origins` | array | Allowed CORS origins |

---

#### `GET /metrics`

Prometheus-style metrics for monitoring.

##### Response (Text)

```
# HELP shard_chat_requests_total Total chat completion requests
# TYPE shard_chat_requests_total counter
shard_chat_requests_total 1234

# HELP shard_chat_failures_total Total inference failures
# TYPE shard_chat_failures_total counter
shard_chat_failures_total 5

# HELP shard_auth_failures_total Total authentication failures
# TYPE shard_auth_failures_total counter
shard_auth_failures_total 2

# HELP shard_rate_limited_total Total rate-limited requests
# TYPE shard_rate_limited_total counter
shard_rate_limited_total 10
```

##### Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `shard_chat_requests_total` | counter | Total chat completion requests |
| `shard_chat_failures_total` | counter | Total inference failures |
| `shard_auth_failures_total` | counter | Total authentication failures |
| `shard_rate_limited_total` | counter | Total rate-limited requests |

---

## Error Codes

The API uses standard HTTP status codes and includes detailed error information in the response body.

### HTTP Status Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 400 | Bad Request |
| 401 | Unauthorized |
| 413 | Payload Too Large |
| 429 | Too Many Requests |
| 500 | Internal Server Error |
| 503 | Service Unavailable |

### Error Response Format

```json
{
  "error": {
    "message": "Error description here",
    "type": "error_type",
    "param": null,
    "code": null
  }
}
```

### Common Errors

#### 400 Bad Request

```json
{
  "detail": "Prompt too large (>16000 chars)"
}
```

#### 401 Unauthorized

```json
{
  "detail": "Missing or invalid API key"
}
```

#### 413 Payload Too Large

```json
{
  "detail": "Prompt too large (>16000 chars)"
}
```

#### 429 Too Many Requests

```json
{
  "detail": "Rate limit exceeded"
}
```

Headers included:
- `X-RateLimit-Limit`: Request limit per minute
- `X-RateLimit-Remaining`: Remaining requests

#### 500 Internal Server Error

```json
{
  "detail": "An unexpected error occurred"
}
```

#### 503 Service Unavailable

```json
{
  "detail": "Inference failed: Rust sidecar unreachable"
}
```

---

## Rate Limiting

The API implements rate limiting to prevent abuse and ensure fair access.

### Configuration

Rate limiting is controlled via the `SHARD_RATE_LIMIT_PER_MINUTE` environment variable (default: 60 requests per minute).

### Rate Limit Headers

Responses include rate limit information:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests per minute |
| `X-RateLimit-Remaining` | Remaining requests in current window |

### Handling Rate Limits

When rate-limited:

1. Wait for the `Retry-After` header (if provided)
2. Implement exponential backoff in your client
3. Consider upgrading to a higher tier (if available)

Example backoff in Python:

```python
import time
import requests

def chat_with_backoff(prompt, max_retries=5):
    for attempt in range(max_retries):
        response = requests.post(
            "https://api.shard.network/v1/chat/completions",
            json={"model": "shard-hybrid", "messages": [{"role": "user", "content": prompt}]}
        )

        if response.status_code == 429:
            wait_time = 2 ** attempt
            print(f"Rate limited. Waiting {wait_time}s...")
            time.sleep(wait_time)
        else:
            return response.json()

    raise Exception("Max retries exceeded")
```

---

## Streaming Responses

Streaming allows you to receive tokens as they are generated, reducing perceived latency.

### Enabling Streaming

Set `stream: true` in your request:

```json
{
  "model": "shard-hybrid",
  "messages": [{"role": "user", "content": "Tell me a story"}],
  "stream": true
}
```

### Parsing SSE Events

Server-Sent Events are sent one per line in the format:

```
data: <json_chunk>\n\n
```

The stream ends with:

```
data: [DONE]\n\n
```

### Complete Streaming Example (Python)

```python
import requests
import json

def stream_chat(prompt):
    response = requests.post(
        "https://api.shard.network/v1/chat/completions",
        headers={"Content-Type": "application/json"},
        json={
            "model": "shard-hybrid",
            "messages": [{"role": "user", "content": prompt}],
            "stream": True,
        },
        stream=True,
    )

    full_content = ""

    for line in response.iter_lines():
        if line:
            line = line.decode('utf-8')
            if line.startswith("data: "):
                data = line[6:]
                if data == "[DONE]":
                    break
                try:
                    chunk = json.loads(data)
                    delta = chunk["choices"][0].get("delta", {})
                    content = delta.get("content", "")
                    if content:
                        full_content += content
                        print(content, end="", flush=True)
                except json.JSONDecodeError:
                    continue

    return full_content

# Usage
story = stream_chat("Write a short story about a robot")
```

### Handling Connection Issues

When using streaming, handle potential connection drops:

```python
import requests
from requests.exceptions import ConnectionError, Timeout

def stream_chat_with_retry(prompt, max_retries=3):
    for attempt in range(max_retries):
        try:
            response = requests.post(
                "https://api.shard.network/v1/chat/completions",
                json={"model": "shard-hybrid", "messages": [{"role": "user", "content": prompt}], "stream": True},
                stream=True,
                timeout=30,
            )
            # Process stream...
            return
        except (ConnectionError, Timeout) as e:
            print(f"Connection error (attempt {attempt + 1}): {e}")
            if attempt < max_retries - 1:
                continue
            raise
```

---

## OpenAPI Specification

The complete OpenAPI 3.0 specification is available at `/openapi.json`:

```bash
curl https://api.shard.network/openapi.json
```

You can also import this specification into tools like:

- [Swagger UI](https://swagger.io/tools/swagger-ui/)
- [Postman](https://www.postman.com/)
- [Insomnia](https://insomnia.rest/)

---

## SDKs and Libraries

While Shard is OpenAI-compatible, you can use the official OpenAI SDKs:

### Python

```bash
pip install openai
```

```python
from openai import OpenAI

client = OpenAI(
    base_url="https://api.shard.network/v1",
    api_key="your-api-key",
)

response = client.chat.completions.create(
    model="shard-hybrid",
    messages=[{"role": "user", "content": "Hello, Shard!"}],
)
print(response.choices[0].message.content)
```

### JavaScript/TypeScript

```bash
npm install openai
```

```javascript
import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'https://api.shard.network/v1',
  apiKey: 'your-api-key',
});

const response = await client.chat.completions.create({
  model: 'shard-hybrid',
  messages: [{ role: 'user', content: 'Hello, Shard!' }],
});

console.log(response.choices[0].message.content);
```

---

## Support

For API-related issues:

- 📖 [Documentation](https://github.com/ShardNetwork/Shard)
- 🐛 [Bug Reports](https://github.com/ShardNetwork/Shard/issues)
- 💬 [Discussions](https://github.com/ShardNetwork/Shard/discussions)

---

## Changelog

For API changes and version history, see [`CHANGELOG.md`](../CHANGELOG.md).
