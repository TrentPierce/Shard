# Authentication

This guide covers API key authentication, rate limiting, quotas, and security best practices for the Shard Oracle API.

## Table of Contents

- [API Key Basics](#api-key-basics)
- [Getting Your API Key](#getting-your-api-key)
- [Using Your API Key](#using-your-api-key)
- [Rate Limiting](#rate-limiting)
- [Rate Limit Headers](#rate-limit-headers)
- [Quotas and Limits](#quotas-and-limits)
- [Rotating API Keys](#rotating-api-keys)
- [Security Best Practices](#security-best-practices)
- [Troubleshooting](#troubleshooting)

---

## API Key Basics

### What is an API Key?

An API key is a unique identifier used to authenticate requests to the Oracle API. It is required for all authenticated endpoints except `/health`.

### Key Format

Shard API keys follow this format:

```
sk-oracle-<random-32-char-string>
```

**Example**:
```
sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz
```

### Key Length

- **Format**: `sk-oracle-` prefix + 32 alphanumeric characters
- **Total length**: 41 characters
- **Storage**: 64 characters if including the prefix

---

## Getting Your API Key

### Method 1: From Environment Variable

The API key is set in the `SHARD_API_KEY` environment variable when the Oracle starts.

```bash
# Check environment variable
echo $SHARD_API_KEY

# Or on Windows
echo %SHARD_API_KEY%
```

### Method 2: From System Status Endpoint

Query the `/system/status` endpoint to retrieve your API key:

```bash
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer YOUR_EXISTING_API_KEY"
```

**Response**:

```json
{
  "status": "operational",
  "api_key": "sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz",
  "node_mode": "oracle",
  ...
}
```

### Method 3: From Startup Logs

The API key is logged to the console when the Oracle starts:

```bash
# Look for this line in the startup logs
[INFO] Oracle API key: sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz
```

### Method 4: Environment Variable Config

If you're deploying your own Oracle, set the API key in the environment:

```bash
# Linux/Mac
export SHARD_API_KEY="sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz"

# Windows
set SHARD_API_KEY=sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz

# Docker
docker run -e SHARD_API_KEY="sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz" shard-oracle
```

### Method 5: Configuration File

In `config.yaml`:

```yaml
api:
  key: "sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz"
```

---

## Using Your API Key

### cURL Example

```bash
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3-70b-bitnet",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Python Example

```python
import requests

response = requests.post(
    "http://localhost:8000/chat/completions",
    headers={
        "Authorization": "Bearer sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz",
        "Content-Type": "application/json"
    },
    json={
        "model": "llama-3-70b-bitnet",
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)

print(response.json())
```

### JavaScript Example

```javascript
const response = await fetch("http://localhost:8000/chat/completions", {
  method: "POST",
  headers: {
    "Authorization": "Bearer sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz",
    "Content-Type": "application/json"
  },
  body: JSON.stringify({
    model: "llama-3-70b-bitnet",
    messages: [{ role: "user", content: "Hello!" }]
  })
});

const result = await response.json();
console.log(result);
```

### Using an API Key File

For security, you can store your API key in a file and read it programmatically:

**API Key File** (`.shard_api_key`):

```
sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz
```

**Python Example**:

```python
with open('.shard_api_key', 'r') as f:
    api_key = f.read().strip()

response = requests.post(
    "http://localhost:8000/chat/completions",
    headers={
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    },
    json={
        "model": "llama-3-70b-bitnet",
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)
```

---

## Rate Limiting

### What is Rate Limiting?

Rate limiting prevents abuse and ensures fair resource allocation across all users. The API enforces rate limits based on endpoint categories.

### Default Rate Limits

| Endpoint Category | Limit | Window | Notes |
|------------------|-------|--------|-------|
| Chat Completions | 10 requests/minute | 1 minute | Maximum throughput |
| Scout Operations | 30 requests/minute | 1 minute | Scout reserve/release |
| System Operations | 100 requests/minute | 1 minute | System status/cluster |

### Per-Endpoint Limits

**Chat Completions** (`POST /chat/completions`):

```bash
# Rate limit: 10 requests/minute
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "llama-3-70b-bitnet", "messages": [...]}'
```

**Scout Operations** (`POST /scouts/reserve`, `POST /scouts/release`, etc.):

```bash
# Rate limit: 30 requests/minute
curl -X POST http://localhost:8000/scouts/reserve \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"scout_id": "scout-1", "reservation_duration": 60}'
```

**System Operations** (`GET /system/status`, `GET /system/cluster`, etc.):

```bash
# Rate limit: 100 requests/minute
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer YOUR_API_KEY"
```

### Burst Requests

Burst requests within the rate limit window are allowed:

```bash
# All 3 requests are allowed (within 1 minute window)
curl http://localhost:8000/chat/completions -H "Authorization: Bearer KEY" -d '{...}' &
curl http://localhost:8000/chat/completions -H "Authorization: Bearer KEY" -d '{...}' &
curl http://localhost:8000/chat/completions -H "Authorization: Bearer KEY" -d '{...}' &
wait
```

After the 1-minute window expires, the rate limit resets.

---

## Rate Limit Headers

All API responses include rate limit headers to help you monitor and manage your API usage.

### Headers Format

```http
X-RateLimit-Limit: 10
X-RateLimit-Remaining: 9
X-RateLimit-Reset: 1697298600
```

### Header Explanations

| Header | Description | Example |
|--------|-------------|---------|
| `X-RateLimit-Limit` | Total allowed requests in current window | `10` |
| `X-RateLimit-Remaining` | Requests remaining in current window | `9` |
| `X-RateLimit-Reset` | Unix timestamp when rate limit resets | `1697298600` |

### Example Response with Headers

```http
HTTP/1.1 200 OK
Content-Type: application/json
X-RateLimit-Limit: 10
X-RateLimit-Remaining: 9
X-RateLimit-Reset: 1697298600

{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1697298500,
  "model": "llama-3-70b-bitnet",
  "choices": [...]
}
```

### Using Rate Limit Headers in Code

**Python Example**:

```python
import requests
import time

response = requests.post(
    "http://localhost:8000/chat/completions",
    headers={
        "Authorization": "Bearer YOUR_API_KEY",
        "Content-Type": "application/json"
    },
    json={
        "model": "llama-3-70b-bitnet",
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)

# Get rate limit info
limit = int(response.headers.get("X-RateLimit-Limit", 10))
remaining = int(response.headers.get("X-RateLimit-Remaining", 10))
reset = int(response.headers.get("X-RateLimit-Reset", 0))

print(f"Limit: {limit}, Remaining: {remaining}, Resets at: {time.ctime(reset)}")

if remaining == 0:
    print("Rate limit reached. Waiting for reset...")
    time.sleep(reset - time.time())
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
    messages: [{ role: "user", content: "Hello!" }]
  })
});

// Get rate limit info
const limit = parseInt(response.headers.get("X-RateLimit-Limit") || "10");
const remaining = parseInt(response.headers.get("X-RateLimit-Remaining") || "10");
const reset = parseInt(response.headers.get("X-RateLimit-Reset") || "0");

console.log(`Limit: ${limit}, Remaining: ${remaining}, Resets at: ${new Date(reset * 1000).toLocaleString()}`);

if (remaining === 0) {
  console.log("Rate limit reached. Waiting for reset...");
  await new Promise(resolve => setTimeout(resolve, (reset - Date.now() / 1000) * 1000));
}
```

---

## Quotas and Limits

### Default Quotas

The following are the default quotas for API keys:

| Resource | Default Limit | Reset Cycle |
|----------|---------------|-------------|
| Chat requests | 10 requests/minute | 1 minute |
| Scout operations | 30 requests/minute | 1 minute |
| System operations | 100 requests/minute | 1 minute |

### Scaling Your Usage

If you need higher limits, contact the Shard team for custom quotas.

**Contact**: support@shard.network

---

## Rotating API Keys

### When to Rotate

Rotate your API key in the following situations:

1. **Compromise**: If you suspect your API key has been exposed
2. **Security Audit**: After a security review
3. **Project Completion**: When ending a project

### How to Rotate

#### Step 1: Generate a New API Key

The Oracle generates a new API key on startup. To rotate programmatically:

```bash
# Restart the Oracle to generate a new API key
pkill shard-oracle
./start-oracle.sh  # Or your startup command
```

#### Step 2: Update Your Applications

Update all applications using the old API key to use the new one:

```python
# Old API key
OLD_KEY = "sk-oracle-old-abc123..."

# New API key
NEW_KEY = "sk-oracle-new-def456..."
```

#### Step 3: Invalidate Old Key

The old API key remains valid until you restart the Oracle. After restart, the old key no longer works.

**Note**: There's no explicit "delete" or "revoke" operation for API keys. Rotation is achieved by generating a new key.

#### Step 4: Verify New Key Works

```bash
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer sk-oracle-new-def456..."
```

---

## Security Best Practices

### 1. Never Share Your API Key

**❌ DON'T**:
```bash
# Commit API key to version control
git commit -m "Add API key"  # BAD!
echo $SHARD_API_KEY >> .gitignore  # Incomplete

# Share in chat/messaging
"Here's my API key: sk-oracle-..."  # BAD!
```

**✅ DO**:
```bash
# Add to .env file
echo "SHARD_API_KEY=sk-oracle-..." >> .env

# Add to .gitignore
echo ".env" >> .gitignore

# Never echo or print in logs
# Use proper logging instead
```

### 2. Use Environment Variables

**Python Example**:

```python
import os
from dotenv import load_dotenv

# Load from .env file
load_dotenv()

# Get API key from environment
api_key = os.getenv("SHARD_API_KEY")
```

### 3. Use an API Key File

**Python Example**:

```python
# Read from file (don't commit the file!)
with open(".shard_api_key", "r") as f:
    api_key = f.read().strip()

# Protect file permissions
os.chmod(".shard_api_key", 0o600)  # Only owner can read/write
```

### 4. Restrict API Key Usage

**Python Example** (IP whitelisting):

```python
import os
from requests import request

api_key = os.getenv("SHARD_API_KEY")

def call_shard_api(url, data):
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    }

    # Restrict to trusted IP (example)
    response = request("POST", url, headers=headers, json=data)

    if response.status_code == 403:
        raise Exception("Request blocked. Your IP may not be whitelisted.")

    return response.json()
```

### 5. Enable Rate Limit Alerts

Monitor your rate limit usage:

```python
import time

def call_shard_api_with_rate_limit(url, data):
    max_retries = 3

    for attempt in range(max_retries):
        response = request("POST", url, headers=headers, json=data)

        remaining = int(response.headers.get("X-RateLimit-Remaining", 0))

        if remaining > 0:
            return response.json()

        # Rate limit reached, wait and retry
        if attempt < max_retries - 1:
            reset = int(response.headers.get("X-RateLimit-Reset", 0))
            wait_time = max(reset - time.time(), 1)
            print(f"Rate limit reached. Waiting {wait_time:.1f} seconds...")
            time.sleep(wait_time)

    raise Exception("Rate limit exceeded after max retries")
```

### 6. Use HTTPS (Production)

In production, always use HTTPS:

```bash
# Wrong (development only)
http://localhost:8000/chat/completions

# Correct (production)
https://oracle.shard.network/chat/completions
```

### 7. Key Expiry (Coming Soon)

Future versions will support API key expiry. Set expiration dates to automatically revoke old keys.

---

## Troubleshooting

### Issue 1: 401 Unauthorized

**Error Response**:

```json
{
  "error": {
    "code": "unauthorized",
    "message": "Invalid or missing API key"
  }
}
```

**Cause**: API key is missing, incorrect, or malformed.

**Solution**:

```bash
# Check you're including the Bearer prefix
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer sk-oracle-abc123..."  # ✅ Correct
  # -H "Authorization: sk-oracle-abc123..."  # ❌ Missing "Bearer"

# Verify API key is correct
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer sk-oracle-abc123..."
```

---

### Issue 2: 429 Too Many Requests

**Error Response**:

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

**Cause**: You've exceeded the rate limit for the endpoint.

**Solution**:

```bash
# Option 1: Wait and retry after X-RateLimit-Reset time
curl http://localhost:8000/chat/completions \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{...}'  # Will fail with 429

# Option 2: Reduce request frequency
# Instead of 10 requests/minute, do 1 request/minute
```

**Python Example**:

```python
import requests
import time

while True:
    response = requests.post(
        "http://localhost:8000/chat/completions",
        headers={
            "Authorization": "Bearer YOUR_API_KEY",
            "Content-Type": "application/json"
        },
        json={"model": "llama-3-70b-bitnet", "messages": [{"role": "user", "content": "Hello!"}]}
    )

    if response.status_code == 429:
        reset = int(response.headers.get("X-RateLimit-Reset", 0))
        wait_time = max(reset - time.time(), 1)
        print(f"Rate limit exceeded. Waiting {wait_time:.1f} seconds...")
        time.sleep(wait_time)
    else:
        print(response.json())
        break
```

---

### Issue 3: Invalid API Key Format

**Error Response**:

```json
{
  "error": {
    "code": "invalid_api_key",
    "message": "API key must start with 'sk-oracle-'"
  }
}
```

**Cause**: API key doesn't follow the correct format.

**Solution**:

```bash
# Correct format: sk-oracle-<32-char-random-string>
sk-oracle-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz

# ✅ Correct format
# ❌ Incorrect (missing prefix)
abc123def456ghi789jkl012mno345pqr678stu901vwx234yz

# ❌ Incorrect (wrong prefix)
sk-openai-abc123def456...
```

---

### Issue 4: Can't Find API Key

**Symptoms**: You don't know your API key.

**Solution**:

```bash
# Option 1: Check environment variable
echo $SHARD_API_KEY

# Option 2: Query /system/status
curl http://localhost:8000/system/status \
  -H "Authorization: Bearer YOUR_OLD_API_KEY"

# Option 3: Check startup logs
tail -f logs/shard.log | grep "API key"

# Option 4: Generate new key by restarting Oracle
pkill shard-oracle
./start-oracle.sh
```

---

### Issue 5: Rate Limit Exceeded Immediately

**Symptoms**: You get 429 immediately after starting the client.

**Cause**: You're making too many requests in quick succession.

**Solution**:

```python
import time

def rate_limited_call(url, data, min_interval=6.0):
    """Make rate-limited calls (10 requests/minute = 6 seconds between requests)"""
    time.sleep(min_interval)  # Wait at least 6 seconds
    response = requests.post(url, headers=headers, json=data)
    return response.json()

# Use the function
result = rate_limited_call(
    "http://localhost:8000/chat/completions",
    data={"model": "llama-3-70b-bitnet", "messages": [...]},
    min_interval=6.0
)
```

---

## API Key Management Tools

### Environment Variable Management

**macOS/Linux**:
```bash
# Add to ~/.bashrc or ~/.zshrc
export SHARD_API_KEY="sk-oracle-..."
source ~/.bashrc
```

**Windows**:
```powershell
# Add to PowerShell profile
[System.Environment]::SetEnvironmentVariable("SHARD_API_KEY", "sk-oracle-...", "User")

# Reload PowerShell
```

### Docker

```dockerfile
# Dockerfile
FROM shard-oracle:latest
ENV SHARD_API_KEY="sk-oracle-abc123..."
```

```bash
# Docker Compose
services:
  oracle:
    image: shard-oracle:latest
    environment:
      - SHARD_API_KEY=sk-oracle-abc123...
```

### Ansible

```yaml
# playbook.yml
- name: Deploy Shard Oracle
  hosts: servers
  tasks:
    - name: Set API key environment variable
      ansible.builtin.set_fact:
        shard_api_key: "sk-oracle-abc123..."
```

---

## Next Steps

- [API Reference](./api-reference.md) - Complete REST API documentation
- [Rate Limits](./api-reference.md#rate-limiting) - Detailed rate limit information
- [Error Codes](./error-codes.md) - HTTP status codes and error types
- [OpenAI Compatibility](./openai-compatibility.md) - Using the API like OpenAI
