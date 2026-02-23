"""Python SDK tests with mock server.

These tests verify the SDK works correctly against a mock HTTP server.
"""

import pytest
import json
from unittest.mock import patch, Mock
import httpx

from shard_sdk import ShardClient, AsyncShardClient
from shard_sdk.models import ChatMessage, ChatCompletionResponse
from shard_sdk.exceptions import ShardConnectionError, ShardAPIError


# Mock responses
MOCK_CHAT_RESPONSE = {
    "id": "chatcmpl-123",
    "object": "chat.completion",
    "created": 1677652288,
    "model": "shard-hybrid",
    "choices": [{
        "index": 0,
        "message": {
            "role": "assistant",
            "content": "Hello! How can I help you today?"
        },
        "finish_reason": "stop"
    }],
    "usage": {
        "prompt_tokens": 10,
        "completion_tokens": 8,
        "total_tokens": 18
    }
}

MOCK_STREAM_CHUNKS = [
    {"id": "chatcmpl-123", "object": "chat.completion.chunk", "created": 1677652288, "model": "shard-hybrid", "choices": [{"index": 0, "delta": {"role": "assistant", "content": "Hello"}, "finish_reason": None}]},
    {"id": "chatcmpl-123", "object": "chat.completion.chunk", "created": 1677652288, "model": "shard-hybrid", "choices": [{"index": 0, "delta": {"content": "!"}, "finish_reason": None}]},
    {"id": "chatcmpl-123", "object": "chat.completion.chunk", "created": 1677652288, "model": "shard-hybrid", "choices": [{"index": 0, "delta": {"content": " How"}, "finish_reason": None}]},
    {"id": "chatcmpl-123", "object": "chat.completion.chunk", "created": 1677652288, "model": "shard-hybrid", "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]},
]


class MockTransport:
    """Mock HTTP transport that returns predefined responses."""
    
    def __init__(self, response_status=200, response_json=None, stream_chunks=None):
        self.response_status = response_status
        self.response_json = response_json or {}
        self.stream_chunks = stream_chunks or []
        self.request = None
        
    def send(self, request, **kwargs):
        self.request = request
        
        # Handle streaming
        if self.stream_chunks:
            # Return a mock response that iterates
            response = Mock(spec=httpx.Response)
            response.status_code = self.response_status
            response.headers = {"content-type": "text/event-stream"}
            response.is_success = self.response_status < 400
            
            # For streaming, we need to return an iterator
            response.iter_lines = lambda: [f"data: {json.dumps(chunk)}" for chunk in self.stream_chunks] + ["data: [DONE]"]
            return response
        
        # Regular response
        response = Mock(spec=httpx.Response)
        response.status_code = self.response_status
        response.json = lambda: self.response_json
        response.text = json.dumps(self.response_json)
        response.is_success = self.response_status < 400
        return response


# ===== SYNC CLIENT TESTS =====

def test_client_initialization():
    """Test that client initializes with correct defaults."""
    client = ShardClient()
    
    assert client.base_url == "http://localhost:9091"
    assert client.timeout == 30.0
    assert client.max_retries == 3
    
    # Check chat.completions exists
    assert hasattr(client, 'chat')
    assert hasattr(client.chat, 'completions')
    
    client.close()


def test_client_with_custom_settings():
    """Test client with custom base_url and api_key."""
    client = ShardClient(
        base_url="http://custom:8080",
        api_key="test-key-123",
        timeout=60.0,
        max_retries=5
    )
    
    assert client.base_url == "http://custom:8080"
    assert client.api_key == "test-key-123"
    assert client.timeout == 60.0
    assert client.max_retries == 5
    
    client.close()


def test_client_context_manager():
    """Test client as context manager."""
    with ShardClient() as client:
        assert client._client is not None
    
    # Client should be closed after exiting context
    # (This is a basic test - actual close depends on implementation)


@patch('httpx.Client')
def test_chat_completion_success(mock_client_class):
    """Test successful chat completion call."""
    # Setup mock
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = MOCK_CHAT_RESPONSE
    mock_response.text = json.dumps(MOCK_CHAT_RESPONSE)
    
    mock_client = Mock()
    mock_client.post.return_value = mock_response
    mock_client_class.return_value = mock_client
    
    # Make request
    client = ShardClient()
    response = client.chat.completions.create(
        messages=[{"role": "user", "content": "Hello"}],
        model="shard-hybrid"
    )
    
    # Verify response
    assert isinstance(response, ChatCompletionResponse)
    assert response.choices[0].message.content == "Hello! How can I help you today?"
    assert response.usage.total_tokens == 18
    
    client.close()


@patch('httpx.Client')
def test_chat_completion_with_string_message(mock_client_class):
    """Test chat completion with string message (auto-wrapped)."""
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = MOCK_CHAT_RESPONSE
    mock_response.text = json.dumps(MOCK_CHAT_RESPONSE)
    
    mock_client = Mock()
    mock_client.post.return_value = mock_response
    mock_client_class.return_value = mock_client
    
    client = ShardClient()
    response = client.chat.completions.create(
        messages="Hello",  # String instead of list
        model="shard-hybrid"
    )
    
    # Should auto-wrap string as user message
    assert response.choices[0].message.content is not None
    
    client.close()


@patch('httpx.Client')
def test_streaming_chunks_arrive_in_order(mock_client_class):
    """Test that streaming chunks arrive in order."""
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.headers = {"content-type": "text/event-stream"}
    mock_response.is_success = True
    mock_response.status_code = 200
    
    # Create a mock that iterates through chunks
    def iter_lines():
        for chunk in MOCK_STREAM_CHUNKS:
            yield f"data: {json.dumps(chunk)}"
        yield "data: [DONE]"
    
    mock_response.iter_lines = iter_lines
    
    mock_client = Mock()
    mock_client.stream.return_value.__enter__ = Mock(return_value=mock_response)
    mock_client.stream.return_value.__exit__ = Mock(return_value=False)
    mock_client_class.return_value = mock_client
    
    client = ShardClient()
    stream = client.chat.completions.create(
        messages=[{"role": "user", "content": "Hello"}],
        model="shard-hybrid",
        stream=True
    )
    
    chunks = list(stream)
    
    # Should have 4 chunks + done
    assert len(chunks) == 4
    
    # First chunk should have "Hello"
    assert chunks[0].choices[0].delta.content == "Hello"
    
    # Last chunk should have finish_reason
    assert chunks[-1].choices[0].finish_reason == "stop"
    
    client.close()


@patch('httpx.Client')
def test_connection_error_raises_shard_connection_error(mock_client_class):
    """Test that connection error raises ShardConnectionError."""
    mock_client = Mock()
    mock_client.post.side_effect = httpx.ConnectError("Connection failed")
    mock_client_class.return_value = mock_client
    
    client = ShardClient()
    
    with pytest.raises(ShardConnectionError) as exc_info:
        client.chat.completions.create(
            messages=[{"role": "user", "content": "Hello"}]
        )
    
    assert "Cannot connect" in str(exc_info.value)
    
    client.close()


@patch('httpx.Client')
def test_api_error_raises_shard_api_error(mock_client_class):
    """Test that API error raises ShardAPIError."""
    error_response = {"error": {"message": "Invalid request", "type": "invalid_request_error"}}
    
    mock_response = Mock()
    mock_response.status_code = 400
    mock_response.text = json.dumps(error_response)
    mock_response.json.return_value = error_response
    
    mock_client = Mock()
    mock_client.post.return_value = mock_response
    mock_client_class.return_value = mock_client
    
    client = ShardClient()
    
    with pytest.raises(ShardAPIError) as exc_info:
        client.chat.completions.create(
            messages=[{"role": "user", "content": "Hello"}]
        )
    
    assert exc_info.value.status_code == 400
    
    client.close()


# ===== ASYNC CLIENT TESTS =====

@pytest.mark.asyncio
async def test_async_client_initialization():
    """Test async client initialization."""
    client = AsyncShardClient()
    
    assert client.base_url == "http://localhost:9091"
    assert client.timeout == 30.0
    
    await client.close()


@pytest.mark.asyncio
async def test_async_chat_completion_success():
    """Test async client successful chat completion."""
    from unittest.mock import patch, AsyncMock
    
    with patch('httpx.AsyncClient') as mock_client_class:
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = MOCK_CHAT_RESPONSE
        mock_response.text = json.dumps(MOCK_CHAT_RESPONSE)
        
        mock_client = AsyncMock()
        mock_client.post.return_value = mock_response
        mock_client_class.return_value = mock_client
        
        client = AsyncShardClient()
        response = await client.chat.completions.create(
            messages=[{"role": "user", "content": "Hello"}]
        )
        
        assert isinstance(response, ChatCompletionResponse)
        assert response.choices[0].message.content == "Hello! How can I help you today?"
        
        await client.close()


@pytest.mark.asyncio
async def test_async_context_manager():
    """Test async client as context manager."""
    async with AsyncShardClient() as client:
        assert client._client is not None


# ===== MODEL TESTS =====

def test_chat_message_model():
    """Test ChatMessage model."""
    msg = ChatMessage(role="user", content="Hello")
    assert msg.role == "user"
    assert msg.content == "Hello"
    
    # Test serialization
    data = msg.model_dump()
    assert data["role"] == "user"
    assert data["content"] == "Hello"


def test_chat_completion_response_model():
    """Test ChatCompletionResponse model."""
    response = ChatCompletionResponse(**MOCK_CHAT_RESPONSE)
    
    assert response.id == "chatcmpl-123"
    assert response.model == "shard-hybrid"
    assert len(response.choices) == 1
    assert response.choices[0].message.content == "Hello! How can I help you today?"
    assert response.usage.total_tokens == 18


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
