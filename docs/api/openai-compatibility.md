# OpenAI API Compatibility

The Shard Oracle API is designed to be **100% compatible** with the [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat). This allows you to use existing OpenAI clients, libraries, and code with minimal modifications.

## Table of Contents

- [Overview](#overview)
- [Endpoint Compatibility](#endpoint-compatibility)
- [Request Format](#request-format)
- [Response Format](#response-format)
- [Model Names](#model-names)
- [Differences from OpenAI](#differences-from-openai)
- [Migration Guide](#migration-guide)
- [Client Examples](#client-examples)

---

## Overview

The Shard Oracle API implements the following OpenAI-compatible endpoints:

| Endpoint | OpenAI Endpoint | Shard Endpoint | Notes |
|----------|----------------|----------------|-------|
| Chat Completions | `/v1/chat/completions` | `/chat/completions` | ✅ Fully compatible |
| Models | `/v1/models` | `/models` | ✅ Fully compatible |
| Chat Completion by ID | `/v1/chat/completions/{id}` | `/chat/completions/{id}` | ✅ Fully compatible |

**Base URL**: The OpenAI-compatible base URL is:

```
http://localhost:8000
```

Or when running in Swarm mode:

```
http://<oracle-ip>:8000
```

---

## Endpoint Compatibility

### 1. Chat Completions (`/chat/completions`)

The primary endpoint is **fully compatible** with OpenAI's `POST /v1/chat/completions`.

#### OpenAI Example:

```bash
curl https://api.openai.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_OPENAI_KEY" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

#### Shard Example:

```bash
curl http://localhost:8000/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_SHARD_KEY" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

**Note**: The only difference is the model name and API key. The request format is identical.

---

### 2. Models (`/models`)

The models endpoint returns a list of available models in the same format as OpenAI.

#### OpenAI Example:

```bash
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer YOUR_OPENAI_KEY"
```

#### Shard Example:

```bash
curl http://localhost:8000/models \
  -H "Authorization: Bearer YOUR_SHARD_KEY"
```

**OpenAI Response**:

```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4",
      "object": "model",
      "created": 1687882411,
      "owned_by": "openai"
    }
  ]
}
```

**Shard Response**:

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

---

### 3. Chat Completion by ID (`/chat/completions/{id}`)

Retrieve a previously generated completion by ID.

#### OpenAI Example:

```bash
curl https://api.openai.com/v1/chat/completions/{id} \
  -H "Authorization: Bearer YOUR_OPENAI_KEY"
```

#### Shard Example:

```bash
curl http://localhost:8000/chat/completions/{id} \
  -H "Authorization: Bearer YOUR_SHARD_KEY"
```

---

## Request Format

### Chat Completions Request

The request format is **identical** to OpenAI's API.

```typescript
interface ChatRequest {
  model: string;                              // Required
  messages: Array<{                           // Required
    role: "system" | "user" | "assistant";
    content: string;
  }>;
  temperature?: number;                       // Optional, 0.0-2.0, default: 0.7
  max_tokens?: number;                        // Optional, minimum: 1, default: 500
  top_p?: number;                             // Optional, 0.0-1.0, default: 1.0
  n?: number;                                 // Optional, default: 1
  stream?: boolean;                           // Optional, default: false
  stop?: string | string[];                   // Optional
  presence_penalty?: number;                  // Optional, -2.0-2.0
  frequency_penalty?: number;                 // Optional, -2.0-2.0
}
```

### Compatibility Examples

#### Example 1: Simple Chat

**OpenAI**:
```bash
curl https://api.openai.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_KEY" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is 2+2?"}
    ],
    "max_tokens": 50
  }'
```

**Shard**:
```bash
curl http://localhost:8000/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $SHARD_KEY" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is 2+2?"}
    ],
    "max_tokens": 50
  }'
```

#### Example 2: Role-based Chat

**OpenAI**:
```bash
curl https://api.openai.com/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_KEY" \
  -d '{
    "model": "gpt-4",
    "messages": [
      {"role": "user", "content": "Tell me a joke."},
      {"role": "assistant", "content": "Why did the chicken cross the road?"},
      {"role": "user", "content": "To get to the other side."}
    ],
    "temperature": 0.9
  }'
```

**Shard**:
```bash
curl http://localhost:8000/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $SHARD_KEY" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [
      {"role": "user", "content": "Tell me a joke."},
      {"role": "assistant", "content": "Why did the chicken cross the road?"},
      {"role": "user", "content": "To get to the other side."}
    ],
    "temperature": 0.9
  }'
```

---

## Response Format

### Chat Completions Response

The response format is **identical** to OpenAI's API.

```typescript
interface ChatResponse {
  id: string;                                          // Completion ID
  object: "chat.completion";                           // Always "chat.completion"
  created: number;                                     // Unix timestamp
  model: string;                                       // Model name
  choices: Array<{                                     // Choices array
    index: number;
    message: {
      role: "system" | "user" | "assistant";
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

### Example Response

**OpenAI Response**:
```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1687882411,
  "model": "gpt-4",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "2+2 equals 4."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 15,
    "completion_tokens": 5,
    "total_tokens": 20
  }
}
```

**Shard Response**:
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
        "content": "2+2 equals 4."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 15,
    "completion_tokens": 5,
    "total_tokens": 20
  }
}
```

---

## Model Names

### Supported Models

| Shard Model | Description | Equivalent OpenAI Model | Notes |
|------------|-------------|-------------------------|-------|
| `llama-3-70b-bitnet` | 1.58-bit quantized Llama 3 70B | `gpt-4` | Production-ready, ~45GB size |

### Listing Available Models

```bash
# Get list of available models
curl http://localhost:8000/models \
  -H "Authorization: Bearer YOUR_SHARD_KEY"
```

---

## Differences from OpenAI

While the API is highly compatible, there are some differences:

### 1. API Key Authentication

**OpenAI**: `Authorization: Bearer sk-...`
**Shard**: `Authorization: Bearer sk-oracle-...`

The API key format is different. Shard API keys start with `sk-oracle-`.

### 2. Model Names

- OpenAI uses `gpt-4`, `gpt-3.5-turbo`, etc.
- Shard uses `llama-3-70b-bitnet`

### 3. Response Time

- OpenAI: Typically 1-3 seconds
- Shard: Typically 2-5 seconds (due to P2P verification)

### 4. Rate Limits

- OpenAI: 3,000 TPM (tokens per minute) for GPT-4
- Shard: Configurable (default: 10 requests/minute for chat)

### 5. Additional Features

Shard offers unique features not available in OpenAI:

- **Scout Mode**: Reserve scouts for speculative decoding
- **Cluster Status**: View swarm topology and load
- **Golden Tickets**: Test scout verification capabilities
- **Lower Cost**: Free API access with Scout mode

### 6. Unavailable OpenAI Features

These OpenAI features are not available in Shard:

- Fine-tuning
- Images/DALL-E
- Embeddings
- Assistants API
- Batch processing

---

## Migration Guide

### Step 1: Update API Key

```bash
# OpenAI
export OPENAI_KEY="sk-..."

# Shard
export SHARD_KEY="sk-oracle-abc123def456"
```

### Step 2: Update Model Name

```bash
# OpenAI
curl https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_KEY" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'

# Shard
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer $SHARD_KEY" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Step 3: Update Base URL

```python
# OpenAI
from openai import OpenAI
client = OpenAI(api_key="sk-...", base_url="https://api.openai.com/v1")

# Shard
from openai import OpenAI
client = OpenAI(api_key="sk-oracle-...", base_url="http://localhost:8000")
```

### Step 4: Test Compatibility

```python
from openai import OpenAI

# Shard client
client = OpenAI(
    api_key="sk-oracle-abc123def456",
    base_url="http://localhost:8000"
)

# Use any OpenAI-compatible code
response = client.chat.completions.create(
    model="llama-3-70b-bitnet",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Explain quantum physics in simple terms."}
    ],
    temperature=0.7,
    max_tokens=500
)

print(response.choices[0].message.content)
```

---

## Client Examples

### Python (OpenAI Client Library)

```python
from openai import OpenAI

# Initialize with Shard base URL
client = OpenAI(
    api_key="sk-oracle-abc123def456",
    base_url="http://localhost:8000"
)

# Chat completion
response = client.chat.completions.create(
    model="llama-3-70b-bitnet",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "What is 2+2?"}
    ],
    temperature=0.7,
    max_tokens=50
)

print(response.choices[0].message.content)
```

### JavaScript (OpenAI SDK)

```javascript
import OpenAI from 'openai';

const client = new OpenAI({
  apiKey: 'sk-oracle-abc123def456',
  baseURL: 'http://localhost:8000'
});

async function main() {
  const response = await client.chat.completions.create({
    model: 'llama-3-70b-bitnet',
    messages: [
      { role: 'system', content: 'You are a helpful assistant.' },
      { role: 'user', content: 'What is 2+2?' }
    ],
    temperature: 0.7,
    max_tokens: 50
  });

  console.log(response.choices[0].message.content);
}

main();
```

### LangChain (OpenAI Compatible)

```python
from langchain_openai import ChatOpenAI

# Shard LangChain client
llm = ChatOpenAI(
    model="llama-3-70b-bitnet",
    openai_api_key="sk-oracle-abc123def456",
    openai_api_base="http://localhost:8000"
)

response = llm.invoke("What is 2+2?")
print(response.content)
```

### LlamaIndex (OpenAI Compatible)

```python
from llama_index.llms.openai_like import OpenAILike

llm = OpenAILike(
    model="llama-3-70b-bitnet",
    api_key="sk-oracle-abc123def456",
    api_base="http://localhost:8000",
    temperature=0.7
)

response = llm.complete("What is 2+2?")
print(response.text)
```

---

## Testing Compatibility

### Use OpenAI SDK to Test Shard

```python
from openai import OpenAI

client = OpenAI(
    api_key="sk-oracle-abc123def456",
    base_url="http://localhost:8000"
)

# Test 1: Simple chat
response = client.chat.completions.create(
    model="llama-3-70b-bitnet",
    messages=[{"role": "user", "content": "Hello!"}]
)
print(f"Response: {response.choices[0].message.content}")

# Test 2: Role-based chat
response = client.chat.completions.create(
    model="llama-3-70b-bitnet",
    messages=[
        {"role": "user", "content": "Tell me a joke."},
        {"role": "assistant", "content": "Why did the chicken cross the road?"},
        {"role": "user", "content": "To get to the other side."}
    ]
)
print(f"Response: {response.choices[0].message.content}")

# Test 3: With parameters
response = client.chat.completions.create(
    model="llama-3-70b-bitnet",
    messages=[{"role": "user", "content": "What is 2+2?"}],
    temperature=0.9,
    max_tokens=50
)
print(f"Response: {response.choices[0].message.content}")

# Test 4: Multiple choices
response = client.chat.completions.create(
    model="llama-3-70b-bitnet",
    messages=[{"role": "user", "content": "Give me 3 names."}],
    n=3,
    temperature=1.2
)
for choice in response.choices:
    print(f"Choice {choice.index}: {choice.message.content}")

print(f"Usage: {response.usage}")
```

---

## OpenAI Feature Compatibility Matrix

| OpenAI Feature | Shard Support | Notes |
|----------------|--------------|-------|
| Chat Completions | ✅ Fully Compatible | Identical API |
| Models List | ✅ Fully Compatible | Identical API |
| Completion by ID | ✅ Fully Compatible | Identical API |
| Streaming | ✅ Supported | See [Streaming Docs](./streaming.md) |
| Function Calling | ⚠️ Limited | Not officially supported |
| Tools | ⚠️ Limited | Not officially supported |
| Vision (Images) | ❌ Not Supported | Coming soon |
| Embeddings | ❌ Not Supported | Coming soon |
| Fine-tuning | ❌ Not Supported | Coming soon |
| Batch Jobs | ❌ Not Supported | Coming soon |

---

## Next Steps

- [API Reference](./api-reference.md) - Complete REST API documentation
- [Authentication](./authentication.md) - API key and rate limiting details
- [Error Codes](./error-codes.md) - HTTP status codes and error types
- [Streaming](./streaming.md) - SSE streaming format and examples
- [Python Client](./python-client.md) - Python client library usage
- [JavaScript Client](./javascript-client.md) - JavaScript client library usage
