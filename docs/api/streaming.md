# Streaming

The Shard Oracle API supports **Server-Sent Events (SSE)** streaming for real-time chat completion responses. This allows you to receive tokens as they are generated, reducing latency and improving user experience.

## Table of Contents

- [Overview](#overview)
- [Streaming Format](#streaming-format)
- [Streaming Endpoint](#streaming-endpoint)
- [Request Format](#request-format)
- [Response Format](#response-format)
- [Event Types](#event-types)
- [Code Examples](#code-examples)
- [Error Handling](#error-handling)
- [Performance Considerations](#performance-considerations)
- [Best Practices](#best-practices)

---

## Overview

### What is Streaming?

Streaming allows you to receive tokens as they are generated, rather than waiting for the entire response to complete. This is particularly useful for:

- **Reduced latency**: Start displaying output immediately
- **Interactive applications**: Update UI in real-time
- **Long responses**: Don't wait for completion to start showing content
- **Better user experience**: See progress and consume content progressively

### Streaming vs. Non-Streaming

**Non-Streaming** (default):

```bash
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": false
  }'

# Waits for entire response (2-5 seconds)
# Then receives complete JSON
```

**Streaming**:

```bash
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'

# Receives tokens as they are generated (real-time)
# Each line is a complete event
```

---

## Streaming Format

### Server-Sent Events (SSE) Protocol

The Shard Oracle API uses SSE, which is a standard HTTP protocol for server-to-client streaming.

**SSE Format**:

```
data: <event-data>

<empty line>

data: <next-event-data>

<empty line>

<empty line>
```

**Example**:

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"!"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

**Key Points**:
- Each event starts with `data:`
- Events are separated by blank lines
- A `[DONE]` signal indicates the end of the stream
- Responses are Line Delimited JSON (LDJSON)

---

## Streaming Endpoint

### POST /chat/completions (streaming)

Generate a chat completion with streaming enabled.

**Tags**: `chat`

**Authentication**: Required

**Rate Limit**: 10 requests/minute

**Request Body**:

```json
{
  "model": "llama-3-70b-bitnet",
  "messages": [
    {
      "role": "user",
      "content": "Explain quantum physics in simple terms."
    }
  ],
  "stream": true,
  "max_tokens": 500,
  "temperature": 0.7
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
  stream?: boolean;              // Required for streaming
  temperature?: number;
  max_tokens?: number;
}
```

---

## Request Format

### Enable Streaming

Set `"stream": true` in the request body:

```json
{
  "model": "llama-3-70b-bitnet",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "What is 2+2?"}
  ],
  "stream": true,
  "max_tokens": 100
}
```

**Note**: Without `"stream": true`, the response is non-streaming (complete JSON).

---

## Response Format

### Streaming Events

Each event is a JSON object with the following structure:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion.chunk",
  "created": 1697298500,
  "model": "llama-3-70b-bitnet",
  "choices": [
    {
      "index": 0,
      "delta": {
        "role": "assistant",
        "content": "Hello"
      },
      "finish_reason": null
    }
  ]
}
```

### Event Types

1. **Chunk Event**: Continuation of the response
2. **Final Chunk**: Last chunk with `finish_reason: "stop"` or `"length"`
3. **DONE Event**: `[DONE]` signal to indicate completion

### Complete Streaming Response Example

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"2"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"+"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"2"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":" = "},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"4"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

---

## Event Types

### 1. Chunk Event

**Description**: Continuation of the response with partial content.

**Fields**:

```typescript
interface ChunkEvent {
  id: string;                    // Completion ID
  object: "chat.completion.chunk"; // Always "chat.completion.chunk"
  created: number;                // Unix timestamp
  model: string;                  // Model name
  choices: Array<{
    index: number;                // Choice index
    delta: {
      role?: string;              // "assistant"
      content?: string;           // Current token(s)
    };
    finish_reason?: null | "stop" | "length";
  }>;
}
```

**Example**:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion.chunk",
  "created": 1697298500,
  "model": "llama-3-70b-bitnet",
  "choices": [{
    "index": 0,
    "delta": {"role": "assistant", "content": "Hello"},
    "finish_reason": null
  }]
}
```

---

### 2. Final Chunk

**Description**: Last chunk indicating completion.

**Fields**:

```typescript
interface FinalChunkEvent extends ChunkEvent {
  choices: Array<{
    index: number;
    delta: {};                    // Empty delta
    finish_reason: "stop" | "length";  // Always present
  }>;
}
```

**Example**:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion.chunk",
  "created": 1697298500,
  "model": "llama-3-70b-bitnet",
  "choices": [{
    "index": 0,
    "delta": {},
    "finish_reason": "stop"
  }]
}
```

---

### 3. DONE Event

**Description**: Special signal indicating the end of the stream.

**Format**:

```
data: [DONE]
```

**Note**: No JSON object, just `[DONE]` string.

---

## Code Examples

### cURL Example

```bash
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [{"role": "user", "content": "Tell me a story about a brave knight."}],
    "stream": true
  }'
```

**Output**:

```
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"Once"}, "finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{"role":"assistant","content":"upon"}, "finish_reason":null}]}

...

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1697298500,"model":"llama-3-70b-bitnet","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

**To Pretty-Print**:

```bash
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "llama-3-70b-bitnet", "messages": [{"role": "user", "content": "Hello!"}], "stream": true}' \
  | while IFS= read -r line; do
    if [[ "$line" != "data: " ]]; then
      echo "$line" | jq . 2>/dev/null || echo "$line"
    fi
  done
```

---

### Python Example

```python
import requests
import json

def stream_chat_completion(api_url, api_key, messages):
    """Stream chat completion from the Shard Oracle API"""

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    }

    data = {
        "model": "llama-3-70b-bitnet",
        "messages": messages,
        "stream": True,
        "max_tokens": 500
    }

    response = requests.post(api_url, headers=headers, json=data, stream=True)

    if response.status_code != 200:
        error = response.json()["error"]
        raise Exception(f"Error: {error['message']}")

    full_content = ""

    for line in response.iter_lines():
        if line:
            line = line.decode("utf-8")
            if line.startswith("data: "):
                data_str = line[6:]  # Remove "data: " prefix

                if data_str == "[DONE]":
                    break

                try:
                    event = json.loads(data_str)

                    # Extract content from the event
                    if event["choices"]:
                        choice = event["choices"][0]
                        delta = choice["delta"]

                        if "content" in delta:
                            chunk = delta["content"]
                            full_content += chunk
                            print(chunk, end="", flush=True)

                except json.JSONDecodeError:
                    continue

    print()  # Newline after completion

    return full_content

# Usage
api_url = "http://localhost:8000/chat/completions"
api_key = "sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz"

messages = [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Tell me a story about a brave knight."}
]

response = stream_chat_completion(api_url, api_key, messages)
print(f"\n\nFull response: {response}")
```

---

### Python with OpenAI SDK

```python
from openai import OpenAI

client = OpenAI(
    api_key="sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz",
    base_url="http://localhost:8000"
)

# Stream with OpenAI SDK
stream = client.chat.completions.create(
    model="llama-3-70b-bitnet",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Tell me a story about a brave knight."}
    ],
    stream=True,
    max_tokens=500
)

for chunk in stream:
    if chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)

print()  # Newline
```

---

### JavaScript/TypeScript Example

```javascript
async function streamChatCompletion(apiUrl, apiKey, messages) {
  const response = await fetch(apiUrl, {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${apiKey}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      model: 'llama-3-70b-bitnet',
      messages: messages,
      stream: true,
      max_tokens: 500
    })
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(`Error: ${error.error.message}`);
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder('utf-8');
  let fullContent = '';

  while (true) {
    const { done, value } = await reader.read();

    if (done) break;

    const chunk = decoder.decode(value);
    const lines = chunk.split('\n');

    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const dataStr = line.slice(6);

        if (dataStr === '[DONE]') {
          break;
        }

        try {
          const event = JSON.parse(dataStr);

          if (event.choices && event.choices[0].delta.content) {
            const chunkContent = event.choices[0].delta.content;
            fullContent += chunkContent;
            process.stdout.write(chunkContent);  // Or update UI
          }
        } catch (e) {
          console.error('Failed to parse event:', e);
        }
      }
    }
  }

  process.stdout.write('\n');  // Newline
  return fullContent;
}

// Usage
const apiUrl = 'http://localhost:8000/chat/completions';
const apiKey = 'sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz';

const messages = [
  { role: 'system', content: 'You are a helpful assistant.' },
  { role: 'user', content: 'Tell me a story about a brave knight.' }
];

streamChatCompletion(apiUrl, apiKey, messages)
  .then(content => console.log('\n\nFull response:', content))
  .catch(error => console.error('Error:', error.message));
```

---

### JavaScript/TypeScript with OpenAI SDK

```javascript
import OpenAI from 'openai';

const client = new OpenAI({
  apiKey: 'sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz',
  baseURL: 'http://localhost:8000'
});

async function main() {
  const stream = await client.chat.completions.create({
    model: 'llama-3-70b-bitnet',
    messages: [
      { role: 'system', content: 'You are a helpful assistant.' },
      { role: 'user', content: 'Tell me a story about a brave knight.' }
    ],
    stream: true,
    max_tokens: 500
  });

  for await (const chunk of stream) {
    if (chunk.choices[0]?.delta?.content) {
      process.stdout.write(chunk.choices[0].delta.content);
    }
  }

  process.stdout.write('\n');
}

main();
```

---

## Error Handling

### Streaming Errors

Errors can occur during streaming. The error response format is the same as non-streaming:

```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded. Try again in 60 seconds.",
    "details": {"retry_after": 60}
  }
}
```

**Handling Streaming Errors**:

```python
import requests
import json

def stream_with_error_handling(api_url, api_key, messages):
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    }

    data = {
        "model": "llama-3-70b-bitnet",
        "messages": messages,
        "stream": True,
        "max_tokens": 500
    }

    try:
        response = requests.post(api_url, headers=headers, json=data, stream=True)

        # Check for HTTP errors
        if response.status_code == 429:
            error = response.json()["error"]
            retry_after = error["details"]["retry_after"]
            print(f"Rate limit exceeded. Waiting {retry_after} seconds...")
            time.sleep(retry_after)
            return stream_with_error_handling(api_url, api_key, messages)  # Retry

        elif response.status_code != 200:
            error = response.json()["error"]
            raise Exception(f"Error: {error['message']}")

        full_content = ""

        for line in response.iter_lines():
            if line:
                line = line.decode("utf-8")
                if line.startswith("data: "):
                    data_str = line[6:]

                    if data_str == "[DONE]":
                        break

                    try:
                        event = json.loads(data_str)
                        if event["choices"]:
                            delta = event["choices"][0].get("delta", {})
                            if "content" in delta:
                                full_content += delta["content"]
                                print(delta["content"], end="", flush=True)

                    except json.JSONDecodeError:
                        continue

        print()
        return full_content

    except requests.exceptions.RequestException as e:
        raise Exception(f"Request failed: {str(e)}")

# Usage
messages = [
    {"role": "user", "content": "Hello!"}
]

response = stream_with_error_handling(
    "http://localhost:8000/chat/completions",
    "sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz",
    messages
)
```

---

## Performance Considerations

### 1. Latency

**Non-Streaming**: 2-5 seconds (wait for entire response)
**Streaming**: 1-2 seconds (first token within 1-2 seconds)

Streaming provides **50% faster perceived latency** because users see output immediately.

### 2. Bandwidth

**Non-Streaming**: Single large JSON response
**Streaming**: Smaller incremental JSON responses

Streaming can be more bandwidth-efficient for long responses.

### 3. Memory

**Non-Streaming**: Store entire response in memory
**Streaming**: Process tokens as they arrive

Streaming is more memory-efficient for long responses.

### 4. Connection Persistence

Streaming uses **HTTP Keep-Alive**, which maintains the connection for efficiency.

---

## Best Practices

### 1. Always Handle Errors

```python
# ✅ Good: Handle rate limits and errors
try:
    response = requests.post(...)
    if response.status_code == 429:
        # Handle rate limit
        pass
except requests.exceptions.RequestException:
    # Handle connection errors
    pass
```

### 2. Process Tokens Incrementally

```python
# ✅ Good: Update UI as tokens arrive
full_content = ""
for chunk in stream:
    full_content += chunk.choices[0].delta.content
    update_ui(full_content)  # Real-time updates
```

### 3. Handle [DONE] Signal

```python
# ✅ Good: Stop reading after [DONE]
for line in response.iter_lines():
    if line.startswith("data: [DONE]"):
        break
```

### 4. Use Streaming for UI

```javascript
// ✅ Good: Stream to UI for real-time experience
streamChatCompletion()
  .then(content => {
    updateUI(content);  // Real-time updates
  });
```

### 5. Optimize Retry Strategy

```python
import time

# ✅ Good: Exponential backoff
max_retries = 3
for attempt in range(max_retries):
    try:
        response = requests.post(...)
        return response
    except:
        if attempt < max_retries - 1:
            time.sleep(2 ** attempt)  # Exponential backoff
            continue
        raise
```

---

## Troubleshooting

### Issue 1: No Output

**Symptom**: No tokens received, connection hangs

**Possible Causes**:
- Incorrect endpoint
- Network connectivity issue
- Incorrect API key

**Solution**:
```bash
# Test with curl
curl http://localhost:8000/health

# Check API key
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer YOUR_API_KEY"
```

---

### Issue 2: Invalid JSON Events

**Symptom**: JSON decode errors

**Cause**: Malformed event data

**Solution**:
```python
try:
    event = json.loads(data_str)
except json.JSONDecodeError:
    continue  # Skip malformed events
```

---

### Issue 3: Slow Streaming

**Symptom**: Tokens arrive slowly

**Cause**:
- High cluster load
- Network lag
- Long verification time

**Solution**:
```python
# Monitor latency
start_time = time.time()
for line in response.iter_lines():
    if line:
        latency = time.time() - start_time
        if latency > 5:  # >5 seconds
            print("Warning: Slow streaming detected")
```

---

## Next Steps

- [API Reference](./api-reference.md) - Complete REST API documentation
- [OpenAI Compatibility](./openai-compatibility.md) - OpenAI API compatibility
- [Python Client](./python-client.md) - Python client library usage
- [JavaScript Client](./javascript-client.md) - JavaScript client library usage
