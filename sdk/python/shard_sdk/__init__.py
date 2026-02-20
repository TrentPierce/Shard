"""Shard SDK — Python client for distributed inference."""

from shard_sdk.client import ShardClient
from shard_sdk.async_client import AsyncShardClient
from shard_sdk.models import (
    ChatMessage,
    ChatCompletionRequest,
    ChatCompletionResponse,
    ChatCompletionChoice,
    StreamDelta,
    Usage,
)
from shard_sdk.exceptions import (
    ShardError,
    ShardAPIError,
    ShardTimeoutError,
    ShardConnectionError,
    ShardAuthError,
)

__version__ = "0.1.0"
__all__ = [
    "ShardClient",
    "AsyncShardClient",
    "ChatMessage",
    "ChatCompletionRequest",
    "ChatCompletionResponse",
    "ChatCompletionChoice",
    "StreamDelta",
    "Usage",
    "ShardError",
    "ShardAPIError",
    "ShardTimeoutError",
    "ShardConnectionError",
    "ShardAuthError",
]
