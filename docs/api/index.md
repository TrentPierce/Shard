# API Documentation Index

Comprehensive API documentation for the Shard network.

## Quick Links

| Document | Description | Status |
|----------|-------------|--------|
| [Overview](./api-overview.md) | High-level architecture and features | ✅ Complete |
| [REST API Reference](./api-reference.md) | Complete REST API endpoints and schemas | ✅ Complete |
| [OpenAI Compatibility](./openai-compatibility.md) | OpenAI API compatibility details | ✅ Complete |
| [Authentication](./authentication.md) | API key authentication and rate limiting | ✅ Complete |
| [Error Codes](./error-codes.md) | HTTP status codes and error types | ✅ Complete |
| [Streaming Responses](./streaming.md) | SSE streaming format and examples | ✅ Complete |
| [Python Client](./python-client.md) | Python client library and usage | ✅ Complete |
| [JavaScript Client](./javascript-client.md) | JavaScript client library and usage | ✅ Complete |
| [cURL Examples](./curl-examples.md) | cURL command examples for all endpoints | ✅ Complete |

---

## REST API Endpoints

### Chat & Inference

| Endpoint | Method | Description | Authentication |
|----------|--------|-------------|---------------|
| `/v1/chat/completions` | POST | Chat completion with streaming support | ✅ Optional |
| `/v1/models` | GET | List available models | ✅ Optional |
| `/v1/system/topology` | GET | Get network topology | ✅ Optional |
| `/v1/system/peers` | GET | List connected peers | ✅ Optional |

### Scout Management

| Endpoint | Method | Description | Authentication |
|----------|--------|-------------|---------------|
| `/v1/scout/reputation/{peer_id}` | GET | Get scout reputation | ✅ Required |
| `/v1/scout/banned` | GET | List banned scouts | ✅ Required |
| `/v1/scout/unban/{peer_id}` | POST | Unban a scout (admin) | ✅ Required |
| `/v1/scout/reset-reputation/{peer_id}` | POST | Reset scout reputation (admin) | ✅ Required |
| `/v1/scout/draft` | POST | Submit scout draft tokens | ✅ Required |
| `/v1/scout/work` | GET | Get work for scouts | ✅ Required |

### System & Health

| Endpoint | Method | Description | Authentication |
|----------|--------|-------------|---------------|
| `/health` | GET | Health check | ✅ Optional |
| `/metrics` | GET | Prometheus metrics | ✅ Optional |

---

## Authentication

### API Key Configuration

Set the `SHARD_API_KEYS` environment variable:

```bash
# Single API key
export SHARD_API_KEYS="sk-your-api-key-here"

# Multiple API keys (comma-separated)
export SHARD_API_KEYS="sk-key1,sk-key2,sk-key3"
```

### Request Headers

Include the API key in one of the following headers:

```bash
# Option 1: Authorization Bearer token
Authorization: Bearer sk-your-api-key-here

# Option 2: X-API-Key header
X-API-Key: sk-your-api-key-here
```

### Authentication Behavior

- **When `SHARD_API_KEYS` is set**: Both header formats accepted, request will be rejected if neither provided
- **When `SHARD_API_KEYS` is not set**: All endpoints accessible (development mode)
- **Anonymous access**: Default if keys not configured

---

## Rate Limiting

### Default Limits

| Endpoint Type | Limit | Window |
|--------------|-------|--------|
| Standard endpoints | 60 req/min | Per IP |
| Scout endpoints | 120 req/min | Per IP |
| Admin endpoints | 60 req/min | Per peer ID |

### Rate Limit Headers

Responses include rate limit information:

```http
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 45
```

### Handling Rate Limits

```bash
# Check remaining requests
curl -H "X-API-Key: sk-..." \
  https://api.shard.network/v1/chat/completions \
  -H "X-RateLimit-Remaining: 45"

# If rate limited, retry with backoff
while True; do
  response=$(curl -s -w "%{http_code}" ...)

  if [ "$response" = "429" ]; then
    wait_time=$((2 ** attempt))
    echo "Rate limited. Waiting ${wait_time}s..."
    sleep $wait_time
    attempt=$((attempt + 1))
  else
    break
  fi
done
```

---

## OpenAI Compatibility

The Shard API is fully compatible with OpenAI's chat completions API.

### Key Features

- ✅ Same request/response format
- ✅ Same streaming format (Server-Sent Events)
- ✅ Same error response format
- ✅ Same model listing endpoint
- ✅ Same token counting
- ✅ Same rate limiting headers

### Supported Models

| Model ID | Description | Implementation |
|----------|-------------|----------------|
| `shard-hybrid` | Hybrid inference (Shard + Scout) | Default model |

### Python Client

```python
from openai import OpenAI

client = OpenAI(
    base_url="https://api.shard.network/v1",
    api_key="sk-your-api-key-here"
)

response = client.chat.completions.create(
    model="shard-hybrid",
    messages=[{"role": "user", "content": "Hello!"}],
    stream=False
)

print(response.choices[0].message.content)
```

---

## Response Format

### Non-Streaming Response

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1704062400,
  "model": "shard-hybrid",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! I am the Shard AI network."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 5,
    "completion_tokens": 12,
    "total_tokens": 17
  }
}
```

### Streaming Response (SSE)

```
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1704062400,"model":"shard-hybrid","choices":[{"index":0,"delta":{"role":"assistant","content":"The"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1704062400,"model":"shard-hybrid","choices":[{"index":0,"delta":{"content":" quantum"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1704062400,"model":"shard-hybrid","choices":[{"index":0,"delta":{"content":" physics"},"finish_reason":"stop"}]}

data: [DONE]
```

---

## Example: Complete Chat Completion

### cURL Example

```bash
curl https://api.shard.network/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-your-api-key-here" \
  -d '{
    "model": "shard-hybrid",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Explain quantum physics"}
    ],
    "temperature": 0.7,
    "max_tokens": 256,
    "stream": false
  }'
```

### Python Example

```python
import requests

response = requests.post(
    "https://api.shard.network/v1/chat/completions",
    headers={
        "Authorization": "Bearer sk-your-api-key-here",
        "Content-Type": "application/json",
    },
    json={
        "model": "shard-hybrid",
        "messages": [
            {"role": "user", "content": "Explain quantum physics"}
        ],
        "max_tokens": 256,
    }
)

result = response.json()
print(result["choices"][0]["message"]["content"])
```

### JavaScript Example

```javascript
const response = await fetch('https://api.shard.network/v1/chat/completions', {
  method: 'POST',
  headers: {
    'Authorization': 'Bearer sk-your-api-key-here',
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    model: 'shard-hybrid',
    messages: [
      { role: 'user', content: 'Explain quantum physics' }
    ],
    max_tokens: 256,
  }),
});

const result = await response.json();
console.log(result.choices[0].message.content);
```

---

## Troubleshooting

### Common Issues

**Q: Getting 401 Unauthorized**
- Check that `SHARD_API_KEYS` environment variable is set correctly
- Verify API key format: `sk-` prefix
- Try alternative header format: `X-API-Key` instead of `Authorization`

**Q: Getting 429 Rate Limited**
- Reduce request rate (wait between requests)
- Check `X-RateLimit-Remaining` header for remaining requests
- Implement exponential backoff

**Q: Getting 500 Internal Server Error**
- Check API status via `/health` endpoint
- Wait a moment and retry
- Report issue if persistent

**Q: Stream not working**
- Ensure `stream: true` is set in request
- Check network connectivity
- Try non-streaming first to isolate the issue

---

## Support

- 📖 [GitHub Repository](https://github.com/ShardNetwork/Shard)
- 🐛 [Bug Reports](https://github.com/ShardNetwork/Shard/issues)
- 💬 [Discussions](https://github.com/ShardNetwork/Shard/discussions)
- 📧 support@shard.network

---

**Last Updated**: 2026-02-12  
**Version**: 1.0  
**API Version**: v1
