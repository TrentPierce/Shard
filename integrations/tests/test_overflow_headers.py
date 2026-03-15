import json
import pytest
from fastapi.testclient import TestClient
from integrations.overflow_router import app, router
import httpx
from unittest.mock import AsyncMock, patch

client = TestClient(app)

@pytest.mark.asyncio
async def test_overflow_routing_headers():
    # Mock the upstream responses
    mock_response = httpx.Response(
        200, 
        content=json.dumps({"choices": [{"message": {"content": "Hello"}}]}).encode(),
        headers={"content-type": "application/json"}
    )
    
    # Patch the AsyncClient.request method
    with patch.object(httpx.AsyncClient, "request", return_value=mock_response) as mock_request:
        # Force routing to shard by setting GPU threshold low
        with patch.object(router, "get_gpu_utilization", return_value=0.9):
            response = client.post(
                "/v1/chat/completions",
                json={"messages": [{"role": "user", "content": "hi"}], "stream": False}
            )
            
            assert response.status_code == 200
            # Check if the router added the headers to the request it sent upstream
            args, kwargs = mock_request.call_args
            assert kwargs["headers"]["x-shard-overflow-routed"] == "true"
            assert kwargs["headers"]["x-shard-overflow-destination"] == "shard"
            
            # Check if the router returned the headers in the response back to us
            assert response.headers["x-shard-overflow-routed"] == "true"
            assert response.headers["x-shard-overflow-destination"] == "shard"

@pytest.mark.asyncio
async def test_primary_routing_headers():
    mock_response = httpx.Response(
        200, 
        content=json.dumps({"choices": [{"message": {"content": "Hello Primary"}}]}).encode(),
        headers={"content-type": "application/json"}
    )
    
    with patch.object(httpx.AsyncClient, "request", return_value=mock_response) as mock_request:
        # Force routing to primary by setting GPU threshold high
        with patch.object(router, "get_gpu_utilization", return_value=0.1):
            response = client.post(
                "/v1/chat/completions",
                json={"messages": [{"role": "user", "content": "hi"}], "stream": False}
            )
            
            assert response.status_code == 200
            args, kwargs = mock_request.call_args
            assert kwargs["headers"]["x-shard-overflow-routed"] == "true"
            assert kwargs["headers"]["x-shard-overflow-destination"] == "primary"
            
            assert response.headers["x-shard-overflow-routed"] == "true"
            assert response.headers["x-shard-overflow-destination"] == "primary"
