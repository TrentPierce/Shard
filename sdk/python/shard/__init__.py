"""Shard SDK — Python client for distributed inference."""

from .client import ShardClient
from .client import ShardClient as Shard
from .async_client import AsyncShardClient
from .async_client import AsyncShardClient as AsyncShard
from .models import (
    ChatMessage,
    ChatCompletionRequest,
    ChatCompletionResponse,
    ChatCompletionChoice,
    StreamDelta,
    Usage,
)
from .exceptions import (
    ShardError,
    ShardAPIError,
    ShardTimeoutError,
    ShardConnectionError,
    ShardAuthError,
)

__version__ = "0.1.0"
__all__ = [
    "Shard",
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
