# Error Codes

This guide covers all HTTP status codes, error codes, and error types returned by the Shard API.

## Table of Contents

- [Error Response Format](#error-response-format)
- [HTTP Status Codes](#http-status-codes)
- [Error Codes](#error-codes)
- [Handling Errors](#handling-errors)
- [Common Errors](#common-errors)
- [Troubleshooting Guide](#troubleshooting-guide)

---

## Error Response Format

All error responses follow a consistent format:

```json
{
  "error": {
    "code": "error_code",
    "message": "Human-readable error message",
    "details": {
      "key": "value"
    },
    "request_id": "req-abc123"
  }
}
```

### Field Explanations

| Field | Description | Example |
|-------|-------------|---------|
| `code` | Machine-readable error code | `invalid_api_key` |
| `message` | Human-readable error description | "API key must start with 'sk-shard-'" |
| `details` | Additional error details (optional) | `{"value": "abc123"}` |
| `request_id` | Unique request ID for tracking | `req-abc123def456` |

### Error Response Examples

**Invalid API Key**:

```json
{
  "error": {
    "code": "invalid_api_key",
    "message": "API key must start with 'sk-shard-'",
    "request_id": "req-abc123"
  }
}
```

**Rate Limit Exceeded**:

```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded. Try again in 60 seconds.",
    "details": {
      "retry_after": 60
    },
    "request_id": "req-def456"
  }
}
```

**Model Not Found**:

```json
{
  "error": {
    "code": "model_not_found",
    "message": "Model 'gpt-4' not found. Available models: llama-3-70b-bitnet",
    "details": {
      "available_models": ["llama-3-70b-bitnet"]
    },
    "request_id": "req-ghi789"
  }
}
```

---

## HTTP Status Codes

### 2xx Success

| Status Code | Meaning | When to Use |
|-------------|---------|-------------|
| `200 OK` | Request successful | Standard GET/POST request succeeded |
| `201 Created` | Resource created | Scout reservation, etc. |
| `204 No Content` | No content | Successful DELETE/UPDATE |

### 4xx Client Errors

| Status Code | Meaning | Error Code | Description |
|-------------|---------|------------|-------------|
| `400 Bad Request` | Invalid request | `invalid_request` | Malformed request body, invalid parameters |
| `401 Unauthorized` | Authentication failed | `unauthorized` | Invalid or missing API key |
| `403 Forbidden` | Access denied | `forbidden` | Insufficient permissions |
| `404 Not Found` | Resource not found | `not_found` | Model, scout, or endpoint not found |
| `422 Unprocessable Entity` | Validation error | `validation_error` | Request validation failed |
| `429 Too Many Requests` | Rate limit exceeded | `rate_limit_exceeded` | Exceeded rate limit quota |
| `500 Internal Server Error` | Server error | `internal_error` | Server-side error |

### 5xx Server Errors

| Status Code | Meaning | Error Code | Description |
|-------------|---------|------------|-------------|
| `500 Internal Server Error` | Server error | `internal_error` | Unexpected server error |
| `503 Service Unavailable` | Service unavailable | `service_unavailable` | Cluster overloaded or down |
| `504 Gateway Timeout` | Gateway timeout | `gateway_timeout` | Scout verification timeout |

---

## Error Codes

### 4xx Client Errors

#### `invalid_request` (400 Bad Request)

**Description**: The request was malformed or contains invalid parameters.

**Example**:

```json
{
  "error": {
    "code": "invalid_request",
    "message": "max_tokens must be greater than 0",
    "request_id": "req-abc123"
  }
}
```

**Common Causes**:
- Missing required fields
- Invalid data types
- Out of range values
- Malformed JSON

**Examples**:

**Missing `messages` field**:
```json
{
  "error": {
    "code": "invalid_request",
    "message": "The 'messages' field is required",
    "request_id": "req-def456"
  }
}
```

**Invalid `temperature` value**:
```json
{
  "error": {
    "code": "invalid_request",
    "message": "temperature must be between 0.0 and 2.0, got -1.5",
    "request_id": "req-ghi789"
  }
}
```

---

#### `unauthorized` (401 Unauthorized)

**Description**: Authentication failed. API key is missing, invalid, or malformed.

**Example**:

```json
{
  "error": {
    "code": "unauthorized",
    "message": "Invalid or missing API key",
    "request_id": "req-jkl012"
  }
}
```

**Common Causes**:
- Missing `Authorization` header
- Invalid API key format
- Wrong API key value
- API key expired (future feature)

**Examples**:

**Missing `Authorization` header**:
```json
{
  "error": {
    "code": "unauthorized",
    "message": "Authorization header is missing",
    "request_id": "req-mno345"
  }
}
```

**Invalid API key format**:
```json
{
  "error": {
    "code": "invalid_api_key",
    "message": "API key must start with 'sk-shard-'",
    "request_id": "req-pqr678"
  }
}
```

---

#### `forbidden` (403 Forbidden)

**Description**: Access denied. You don't have permission to access this resource.

**Example**:

```json
{
  "error": {
    "code": "forbidden",
    "message": "You don't have permission to access this endpoint",
    "request_id": "req-stu901"
  }
}
```

**Common Causes**:
- Wrong API key
- System endpoint without admin access
- Rate limit exceeded

**Examples**:

**Restart endpoint without admin role**:
```json
{
  "error": {
    "code": "forbidden",
    "message": "Restart operation requires admin privileges",
    "request_id": "req-vwx234"
  }
}
```

---

#### `not_found` (404 Not Found)

**Description**: The requested resource doesn't exist.

**Example**:

```json
{
  "error": {
    "code": "not_found",
    "message": "Scout 'scout-999' not found",
    "request_id": "req-yza567"
  }
}
```

**Common Causes**:
- Non-existent model
- Non-existent scout
- Non-existent reservation
- Non-existent endpoint

**Examples**:

**Model not found**:
```json
{
  "error": {
    "code": "model_not_found",
    "message": "Model 'gpt-4' not found. Available models: llama-3-70b-bitnet",
    "details": {
      "available_models": ["llama-3-70b-bitnet"]
    },
    "request_id": "req-bcd890"
  }
}
```

**Scout not found**:
```json
{
  "error": {
    "code": "not_found",
    "message": "Scout 'scout-999' not found",
    "request_id": "req-efg012"
  }
}
```

**Reservation not found**:
```json
{
  "error": {
    "code": "not_found",
    "message": "Reservation 'reservation-999' not found",
    "request_id": "req-ghi345"
  }
}
```

---

#### `validation_error` (422 Unprocessable Entity)

**Description**: Request validation failed. The request body doesn't meet validation rules.

**Example**:

```json
{
  "error": {
    "code": "validation_error",
    "message": "reservation_duration must be at least 30 seconds",
    "details": {
      "field": "reservation_duration",
      "value": 10,
      "minimum": 30
    },
    "request_id": "req-jkl678"
  }
}
```

**Common Causes**:
- Invalid parameter values
- Missing required fields
- Data type mismatches
- Out of range values

**Examples**:

**Invalid reservation duration**:
```json
{
  "error": {
    "code": "validation_error",
    "message": "reservation_duration must be at least 30 seconds",
    "details": {
      "field": "reservation_duration",
      "value": 10,
      "minimum": 30
    },
    "request_id": "req-mno901"
  }
}
```

**Invalid scout status**:
```json
{
  "error": {
    "code": "validation_error",
    "message": "scout must be 'active' or 'idle', got 'offline'",
    "details": {
      "field": "scout_status",
      "value": "offline",
      "valid_values": ["active", "idle"]
    },
    "request_id": "req-pqr234"
  }
}
```

---

#### `rate_limit_exceeded` (429 Too Many Requests)

**Description**: Rate limit exceeded. You've exceeded the allowed number of requests.

**Example**:

```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded. Try again in 60 seconds.",
    "details": {
      "retry_after": 60
    },
    "request_id": "req-stu567"
  }
}
```

**Common Causes**:
- Too many requests in a short time
- Exceeded rate limit quota
- No more requests remaining

**Examples**:

**Chat completions rate limit**:
```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Chat completions rate limit exceeded. Try again in 10 seconds.",
    "details": {
      "endpoint": "chat/completions",
      "limit": 10,
      "remaining": 0,
      "retry_after": 10
    },
    "request_id": "req-vwx890"
  }
}
```

**Scout operations rate limit**:
```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Scout operations rate limit exceeded. Try again in 5 seconds.",
    "details": {
      "endpoint": "scouts/reserve",
      "limit": 30,
      "remaining": 0,
      "retry_after": 5
    },
    "request_id": "req-yza012"
  }
}
```

---

### 5xx Server Errors

#### `internal_error` (500 Internal Server Error)

**Description**: Unexpected server error. Please try again later.

**Example**:

```json
{
  "error": {
    "code": "internal_error",
    "message": "An internal server error occurred. Please try again later.",
    "request_id": "req-abc456"
  }
}
```

**Common Causes**:
- Server-side bug
- Database connection failure
- Unexpected exception
- Scout verification failure

**Examples**:

**Server processing error**:
```json
{
  "error": {
    "code": "internal_error",
    "message": "Failed to process chat completion request",
    "details": {
      "error": "Traceback (most recent call last):...",
      "request_id": "req-def789"
    },
    "request_id": "req-def789"
  }
}
```

---

#### `service_unavailable` (503 Service Unavailable)

**Description**: Service is temporarily unavailable due to high load or maintenance.

**Example**:

```json
{
  "error": {
    "code": "service_unavailable",
    "message": "Service unavailable. Cluster is overloaded or under maintenance.",
    "details": {
      "message": "System maintenance in progress"
    },
    "request_id": "req-ghi012"
  }
}
```

**Common Causes**:
- Cluster overloaded
- Maintenance mode active
- Scout network unavailable
- Shard daemon down

**Examples**:

**Cluster overload**:
```json
{
  "error": {
    "code": "service_unavailable",
    "message": "Service unavailable. Cluster load > 90%. Please try again later.",
    "details": {
      "cluster_load": 92,
      "available_scouts": 1,
      "total_scouts": 3
    },
    "request_id": "req-jkl345"
  }
}
```

---

#### `gateway_timeout` (504 Gateway Timeout)

**Description**: Scout verification timeout. The scout took too long to respond.

**Example**:

```json
{
  "error": {
    "code": "gateway_timeout",
    "message": "Request timed out after 30 seconds",
    "details": {
      "timeout_seconds": 30,
      "scout_id": "scout-1"
    },
    "request_id": "req-mno678"
  }
}
```

**Common Causes**:
- Scout network lag
- Scout processing delay
- High cluster load
- Network connectivity issues

**Examples**:

**Scout timeout**:
```json
{
  "error": {
    "code": "gateway_timeout",
    "message": "Scout verification timed out after 30 seconds",
    "details": {
      "timeout_seconds": 30,
      "scout_id": "scout-1",
      "expected_time_ms": 25
    },
    "request_id": "req-pqr901"
  }
}
```

---

## Handling Errors

### Python Example

```python
import requests
import time
from requests.exceptions import RequestException

def call_shard_api(url, headers, data, max_retries=3):
    """Call the Shard API with error handling"""

    for attempt in range(max_retries):
        try:
            response = requests.post(url, headers=headers, json=data, timeout=60)

            # Handle HTTP errors
            if response.status_code == 400:
                error = response.json()["error"]
                raise ValueError(f"Bad request: {error['message']}")
            elif response.status_code == 401:
                error = response.json()["error"]
                raise PermissionError(f"Unauthorized: {error['message']}")
            elif response.status_code == 404:
                error = response.json()["error"]
                raise FileNotFoundError(f"Not found: {error['message']}")
            elif response.status_code == 429:
                error = response.json()["error"]
                retry_after = error.get("details", {}).get("retry_after", 60)
                print(f"Rate limit exceeded. Waiting {retry_after} seconds...")
                time.sleep(retry_after)
                continue
            elif response.status_code == 500:
                error = response.json()["error"]
                raise RuntimeError(f"Internal error: {error['message']}")
            elif response.status_code == 503:
                error = response.json()["error"]
                raise TimeoutError(f"Service unavailable: {error['message']}")
            elif response.status_code == 504:
                error = response.json()["error"]
                raise TimeoutError(f"Gateway timeout: {error['message']}")

            # Successful response
            return response.json()

        except RequestException as e:
            if attempt < max_retries - 1:
                time.sleep(2 ** attempt)  # Exponential backoff
                continue
            raise

    raise RuntimeError(f"Max retries ({max_retries}) exceeded")

# Usage
try:
    result = call_shard_api(
        "http://localhost:8000/chat/completions",
        headers={
            "Authorization": "Bearer YOUR_API_KEY",
            "Content-Type": "application/json"
        },
        data={
            "model": "llama-3-70b-bitnet",
            "messages": [{"role": "user", "content": "Hello!"}]
        }
    )
    print(result)

except ValueError as e:
    print(f"Invalid request: {e}")
except PermissionError as e:
    print(f"Unauthorized: {e}")
except FileNotFoundError as e:
    print(f"Resource not found: {e}")
except TimeoutError as e:
    print(f"Timeout: {e}")
except RuntimeError as e:
    print(f"Server error: {e}")
```

### JavaScript Example

```javascript
async function callShardAPI(url, headers, data, maxRetries = 3) {
  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      const response = await fetch(url, {
        method: 'POST',
        headers: headers,
        body: JSON.stringify(data)
      });

      // Handle HTTP errors
      if (response.status === 400) {
        const error = await response.json();
        throw new Error(`Bad request: ${error.error.message}`);
      } else if (response.status === 401) {
        const error = await response.json();
        throw new Error(`Unauthorized: ${error.error.message}`);
      } else if (response.status === 404) {
        const error = await response.json();
        throw new Error(`Not found: ${error.error.message}`);
      } else if (response.status === 429) {
        const error = await response.json();
        const retryAfter = error.error.details?.retry_after || 60;
        console.log(`Rate limit exceeded. Waiting ${retryAfter} seconds...`);
        await new Promise(resolve => setTimeout(resolve, retryAfter * 1000));
        continue;
      } else if (response.status === 500) {
        const error = await response.json();
        throw new Error(`Internal error: ${error.error.message}`);
      } else if (response.status === 503) {
        const error = await response.json();
        throw new Error(`Service unavailable: ${error.error.message}`);
      } else if (response.status === 504) {
        const error = await response.json();
        throw new Error(`Gateway timeout: ${error.error.message}`);
      }

      // Successful response
      return await response.json();

    } catch (e) {
      if (attempt < maxRetries - 1) {
        await new Promise(resolve => setTimeout(resolve, 2 ** attempt * 1000));
        continue;
      }
      throw e;
    }
  }

  throw new Error(`Max retries (${maxRetries}) exceeded`);
}

// Usage
callShardAPI(
  'http://localhost:8000/chat/completions',
  {
    'Authorization': 'Bearer YOUR_API_KEY',
    'Content-Type': 'application/json'
  },
  {
    model: 'llama-3-70b-bitnet',
    messages: [{role: 'user', content: 'Hello!'}]
  }
).then(result => {
  console.log(result);
}).catch(error => {
  console.error(error.message);
});
```

---

## Common Errors

### Error 1: Invalid API Key

**Error**:
```json
{
  "error": {
    "code": "unauthorized",
    "message": "Invalid or missing API key"
  }
}
```

**Solution**:
- Verify you're including the `Authorization: Bearer <key>` header
- Check your API key format: `sk-shard-<32-char-string>`
- Generate a new key if needed

---

### Error 2: Rate Limit Exceeded

**Error**:
```json
{
  "error": {
    "code": "rate_limit_exceeded",
    "message": "Rate limit exceeded. Try again in 60 seconds.",
    "details": {"retry_after": 60}
  }
}
```

**Solution**:
- Wait until `retry_after` seconds have passed
- Reduce request frequency
- Upgrade to a higher quota

---

### Error 3: Model Not Found

**Error**:
```json
{
  "error": {
    "code": "model_not_found",
    "message": "Model 'gpt-4' not found",
    "details": {
      "available_models": ["llama-3-70b-bitnet"]
    }
  }
}
```

**Solution**:
- List available models: `GET /models`
- Use the correct model name

---

### Error 4: Scout Not Found

**Error**:
```json
{
  "error": {
    "code": "not_found",
    "message": "Scout 'scout-999' not found"
  }
}
```

**Solution**:
- List all scouts: `GET /scouts`
- Use a valid scout ID

---

### Error 5: Invalid Parameter

**Error**:
```json
{
  "error": {
    "code": "validation_error",
    "message": "max_tokens must be greater than 0",
    "details": {
      "field": "max_tokens",
      "value": 0,
      "minimum": 1
    }
  }
}
```

**Solution**:
- Check parameter values
- Ensure parameters are in valid range

---

## Troubleshooting Guide

### Checklist for Troubleshooting

When encountering errors, follow this checklist:

#### 1. Check HTTP Status Code
- `4xx`: Client error → Check request format, API key, parameters
- `5xx`: Server error → Wait and retry, check server logs

#### 2. Check Error Code
- `unauthorized`: Check API key
- `rate_limit_exceeded`: Wait for reset time
- `not_found`: Check resource ID
- `validation_error`: Check parameter values

#### 3. Check Request ID
- Use `request_id` in error response for debugging
- Include request ID in bug reports

#### 4. Check Headers
- Verify `Content-Type: application/json`
- Verify `Authorization: Bearer <key>`

#### 5. Check Logs
- Check server logs for detailed error information
- Check scout logs for verification errors

---

### Getting Help

If you encounter an error that isn't documented here:

1. **Check Error Details**: Look at the `details` field for more information
2. **Check Request ID**: Use `request_id` for tracking
3. **Check Logs**: Review server and scout logs
4. **Contact Support**: support@shard.network

**Include in your support request**:
- Error code and message
- Request ID from error response
- HTTP status code
- Reproducible steps
- Expected vs. actual behavior

---

## Next Steps

- [API Reference](./api-reference.md) - Complete REST API documentation
- [Authentication](./authentication.md) - API key and rate limit details
- [Rate Limits](./api-reference.md#rate-limiting) - Detailed rate limit information
- [OpenAI Compatibility](./openai-compatibility.md) - Using the API like OpenAI
