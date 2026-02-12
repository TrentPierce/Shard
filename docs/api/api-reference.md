# REST API Reference

This section provides comprehensive documentation for all REST API endpoints exposed by the Shard Oracle API.

## Table of Contents

- [Base URL](#base-url)
- [Authentication](#authentication)
- [Rate Limiting](#rate-limiting)
- [API Endpoints](#api-endpoints)

---

## Base URL

The base URL for all API requests is:

```
http://localhost:8000
```

Or when running in Swarm mode:

```
http://<oracle-ip>:8000
```

---

## Authentication

The Oracle API uses API key authentication. Include your API key in the `Authorization` header:

```
Authorization: Bearer <your-api-key>
```

### Getting an API Key

The API key is automatically generated when the Oracle starts. You can retrieve it from:

1. **Environment Variable**: Check `SHARD_API_KEY` in the Oracle's environment
2. **System Status**: Use the `/system/status` endpoint
3. **Startup Logs**: The API key is logged to `console.log`

```bash
# Example: Check system status to get API key
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer YOUR_API_KEY"
```

**Security Note**: Store your API key securely. Never commit it to version control.

---

## Rate Limiting

The API enforces rate limiting to prevent abuse and ensure fair resource allocation.

### Default Rate Limits

| Endpoint Category | Limit | Window |
|------------------|-------|--------|
| Chat Completions | 10 requests/minute | 1 minute |
| Scout Operations | 30 requests/minute | 1 minute |
| System Operations | 100 requests/minute | 1 minute |

### Rate Limit Headers

All API responses include rate limit headers:

```http
X-RateLimit-Limit: 10
X-RateLimit-Remaining: 9
X-RateLimit-Reset: 1697298600
```

**429 Too Many Requests**: If you exceed the rate limit, you'll receive a `429` status code. Wait until the `X-RateLimit-Reset` time before retrying.

---

## API Endpoints

### Chat Endpoints

#### POST /chat/completions

Generate a chat completion using the Oracle's 1.58-bit quantized model.

**Tags**: `chat`

**Authentication**: Required

**Rate Limit**: 10 requests/minute

**Request Body**:

```json
{
  "model": "llama-3-70b-bitnet",
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful AI assistant."
    },
    {
      "role": "user",
      "content": "Explain quantum physics in simple terms."
    }
  ],
  "temperature": 0.7,
  "max_tokens": 500,
  "stream": false
}
```

**Schema**:

```typescript
interface ChatRequest {
  model: string;
  messages: Array<{
    role: "system" | "user" | "assistant";
    content: string;
  }>;
  temperature?: number;      // 0.0 to 2.0, default: 0.7
  max_tokens?: number;       // Minimum: 1, default: 500
  stream?: boolean;          // default: false
}
```

**Response**:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1697298500,
  "model": "llama-3-70b-bitnet",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Quantum physics is the study of matter and energy at the most fundamental level..."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 50,
    "completion_tokens": 100,
    "total_tokens": 150
  }
}
```

**Schema**:

```typescript
interface ChatResponse {
  id: string;
  object: "chat.completion";
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: {
      role: string;
      content: string;
    };
    finish_reason: "stop" | "length" | "tool_calls";
  }>;
  usage: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}
```

**cURL Example**:

```bash
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [
      {
        "role": "user",
        "content": "What is the capital of France?"
      }
    ],
    "max_tokens": 50
  }'
```

**Python Example**:

```python
import requests

response = requests.post(
    "http://localhost:8000/chat/completions",
    headers={
        "Authorization": "Bearer YOUR_API_KEY",
        "Content-Type": "application/json"
    },
    json={
        "model": "llama-3-70b-bitnet",
        "messages": [
            {"role": "user", "content": "What is the capital of France?"}
        ],
        "max_tokens": 50
    }
)

result = response.json()
print(result["choices"][0]["message"]["content"])
```

**JavaScript Example**:

```javascript
const response = await fetch("http://localhost:8000/chat/completions", {
  method: "POST",
  headers: {
    "Authorization": "Bearer YOUR_API_KEY",
    "Content-Type": "application/json"
  },
  body: JSON.stringify({
    model: "llama-3-70b-bitnet",
    messages: [
      {role: "user", content: "What is the capital of France?"}
    ],
    max_tokens: 50
  })
});

const result = await response.json();
console.log(result.choices[0].message.content);
```

---

#### GET /models

Retrieve available models and their metadata.

**Tags**: `chat`

**Authentication**: Required

**Response**:

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama-3-70b-bitnet",
      "object": "model",
      "created": 1697298000,
      "owned_by": "shard"
    }
  ]
}
```

**cURL Example**:

```bash
curl http://localhost:8000/models \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

#### GET /chat/completions/{id}

Retrieve a previously generated chat completion by ID.

**Tags**: `chat`

**Authentication**: Required

**Rate Limit**: 60 requests/minute

**Path Parameters**:

- `id` (string): The completion ID

**Response**:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1697298500,
  "model": "llama-3-70b-bitnet",
  "choices": [...],
  "usage": {...}
}
```

**cURL Example**:

```bash
curl http://localhost:8000/chat/completions/chatcmpl-abc123 \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

### Scout Endpoints

#### GET /scouts

List all available scouts in the swarm.

**Tags**: `scouts`

**Authentication**: Required

**Rate Limit**: 30 requests/minute

**Response**:

```json
{
  "object": "list",
  "data": [
    {
      "id": "scout-1",
      "status": "active",
      "ip": "192.168.1.100",
      "port": 8080,
      "capabilities": ["gpu", "webgpu"],
      "load": 0.45
    },
    {
      "id": "scout-2",
      "status": "active",
      "ip": "192.168.1.101",
      "port": 8080,
      "capabilities": ["webgpu"],
      "load": 0.67
    }
  ]
}
```

**cURL Example**:

```bash
curl http://localhost:8000/scouts \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

#### POST /scouts/reserve

Reserve a scout for speculative decoding work.

**Tags**: `scouts`

**Authentication**: Required

**Rate Limit**: 30 requests/minute

**Request Body**:

```json
{
  "scout_id": "scout-1",
  "reservation_duration": 60
}
```

**Schema**:

```typescript
interface ScoutReservationRequest {
  scout_id: string;
  reservation_duration: number;  // seconds, minimum: 30
}
```

**Response**:

```json
{
  "id": "reservation-123",
  "scout_id": "scout-1",
  "reserved_at": 1697298500,
  "expires_at": 1697298560,
  "status": "reserved"
}
```

**cURL Example**:

```bash
curl -X POST http://localhost:8000/scouts/reserve \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "scout_id": "scout-1",
    "reservation_duration": 60
  }'
```

---

#### POST /scouts/release

Release a previously reserved scout.

**Tags**: `scouts`

**Authentication**: Required

**Rate Limit**: 30 requests/minute

**Request Body**:

```json
{
  "reservation_id": "reservation-123"
}
```

**cURL Example**:

```bash
curl -X POST http://localhost:8000/scouts/release \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "reservation_id": "reservation-123"
  }'
```

---

#### POST /scouts/cancel

Cancel an active scout reservation.

**Tags**: `scouts`

**Authentication**: Required

**Rate Limit**: 30 requests/minute

**Request Body**:

```json
{
  "reservation_id": "reservation-123"
}
```

**cURL Example**:

```bash
curl -X POST http://localhost:8000/scouts/cancel \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "reservation_id": "reservation-123"
  }'
```

---

#### GET /scouts/{id}/status

Get detailed status of a specific scout.

**Tags**: `scouts`

**Authentication**: Required

**Rate Limit**: 30 requests/minute

**Path Parameters**:

- `id` (string): The scout ID

**Response**:

```json
{
  "id": "scout-1",
  "status": "active",
  "ip": "192.168.1.100",
  "port": 8080,
  "capabilities": ["gpu", "webgpu"],
  "load": 0.45,
  "tasks_completed": 1234,
  "tasks_failed": 5,
  "avg_latency_ms": 25,
  "uptime_seconds": 360000
}
```

**cURL Example**:

```bash
curl http://localhost:8000/scouts/scout-1/status \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

#### GET /scouts/{id}/logs

Retrieve logs from a specific scout.

**Tags**: `scouts`

**Authentication**: Required

**Rate Limit**: 30 requests/minute

**Path Parameters**:

- `id` (string): The scout ID

**Query Parameters**:

- `limit` (number): Number of logs to return, default: 100
- `level` (string): Filter by log level (DEBUG, INFO, WARN, ERROR), default: all

**Response**:

```json
{
  "scout_id": "scout-1",
  "logs": [
    {
      "timestamp": "2024-10-15T14:30:00Z",
      "level": "INFO",
      "message": "Connected to Oracle"
    },
    {
      "timestamp": "2024-10-15T14:30:01Z",
      "level": "INFO",
      "message": "Work request received"
    }
  ]
}
```

**cURL Example**:

```bash
# Get last 50 logs
curl "http://localhost:8000/scouts/scout-1/logs?limit=50" \
  -H "Authorization: Bearer YOUR_API_KEY"

# Get only ERROR logs
curl "http://localhost:8000/scouts/scout-1/logs?level=ERROR" \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

### System Endpoints

#### GET /system/status

Get comprehensive system status and health information.

**Tags**: `system`

**Authentication**: Required

**Rate Limit**: 100 requests/minute

**Response**:

```json
{
  "status": "operational",
  "api_key": "sk-oracle-abc123def456",
  "node_mode": "oracle",
  "node_type": "oracle_titan",
  "cluster": {
    "oracles": 1,
    "scouts": 3,
    "leeches": 10
  },
  "models": [
    {
      "name": "llama-3-70b-bitnet",
      "size_gb": 45,
      "vram_required_gb": 8
    }
  ],
  "performance": {
    "avg_latency_ms": 28,
    "requests_per_minute": 245,
    "throughput_tokens_per_second": 850
  },
  "uptime_seconds": 86400
}
```

**cURL Example**:

```bash
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

#### GET /system/cluster

Get detailed cluster topology and load distribution.

**Tags**: `system`

**Authentication**: Required

**Rate Limit**: 100 requests/minute

**Response**:

```json
{
  "mode": "swarm",
  "total_nodes": 14,
  "nodes": {
    "oracles": [
      {
        "id": "oracle-1",
        "ip": "192.168.1.1",
        "port": 8000,
        "status": "active",
        "load": 0.75
      }
    ],
    "scouts": [
      {
        "id": "scout-1",
        "ip": "192.168.1.100",
        "port": 8080,
        "status": "active",
        "load": 0.45
      },
      {
        "id": "scout-2",
        "ip": "192.168.1.101",
        "port": 8080,
        "status": "active",
        "load": 0.67
      },
      {
        "id": "scout-3",
        "ip": "192.168.1.102",
        "port": 8080,
        "status": "idle",
        "load": 0.0
      }
    ],
    "leeches": 10
  },
  "load_balancing": {
    "strategy": "least_loaded",
    "target_scouts_per_request": 3
  }
}
```

**cURL Example**:

```bash
curl http://localhost:8000/system/cluster \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

#### POST /system/restart

Restart the cluster (Oracle daemon + scouts).

**Tags**: `system`

**Authentication**: Required (Admin level)

**Rate Limit**: 10 requests/hour

**Response**:

```json
{
  "status": "restart_requested",
  "message": "Cluster restart initiated",
  "restart_in_seconds": 5
}
```

**cURL Example**:

```bash
curl -X POST http://localhost:8000/system/restart \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json"
```

---

#### GET /health

Simple health check endpoint.

**Tags**: `system`

**Authentication**: Not required

**Rate Limit**: Unlimited

**Response**:

```json
{
  "status": "healthy",
  "timestamp": "2024-10-15T14:30:00Z",
  "version": "0.4.0"
}
```

**cURL Example**:

```bash
curl http://localhost:8000/health
```

---

## Response Format

### Success Response (2xx)

All successful responses return JSON data in the following format:

```json
{
  "status": "success",
  "data": { /* response-specific data */ }
}
```

### Error Response (4xx/5xx)

All error responses follow this format:

```json
{
  "error": {
    "code": "error_code",
    "message": "Human-readable error message",
    "details": { /* additional details */ }
  }
}
```

---

## Error Codes

For detailed error codes, see [Error Codes Documentation](./error-codes.md).

---

## OpenAI Compatibility

For OpenAI API compatibility details, see [OpenAI Compatibility Documentation](./openai-compatibility.md).

---

## Authentication Details

For API key authentication and security, see [Authentication Documentation](./authentication.md).

---

## Streaming Support

For SSE streaming format and examples, see [Streaming Documentation](./streaming.md).

---

## Client Libraries

For Python and JavaScript client library usage, see:
- [Python Client Documentation](./python-client.md)
- [JavaScript Client Documentation](./javascript-client.md)

---

## Troubleshooting

For common issues and solutions, see the [Troubleshooting Guide](https://github.com/shard-network/shard/issues?q=is%3Aissue+is%3Aopen).

---

## Support

- **Documentation**: https://docs.shard.network
- **GitHub**: https://github.com/shard-network/shard
- **Discord**: https://discord.gg/shard-network
- **Email**: support@shard.network

---

## Rate Limit Exceeded (429)

If you receive a `429 Too Many Requests` response:

```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded. Try again in 60 seconds.",
    "details": {
      "retry_after": 60
    }
  }
}
```

**Solution**: Wait until `X-RateLimit-Reset` time before retrying.

---

## API Key Missing (401)

If you receive a `401 Unauthorized` response:

```json
{
  "error": {
    "code": "unauthorized",
    "message": "Invalid or missing API key"
  }
}
```

**Solution**: Include the `Authorization: Bearer <your-api-key>` header in your requests.

---

## Next Steps

- [OpenAI Compatibility](./openai-compatibility.md)
- [Authentication](./authentication.md)
- [Error Codes](./error-codes.md)
- [Streaming](./streaming.md)
- [Python Client](./python-client.md)
- [JavaScript Client](./javascript-client.md)
