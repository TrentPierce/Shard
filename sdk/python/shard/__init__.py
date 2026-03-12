from shard.client import Client
from shard.errors import (
    AuthenticationError,
    ConnectionError,
    InsufficientCreditsError,
    InvalidRequestError,
    MeshDegradedError,
    RateLimitError,
    ServerError,
    ShardError,
)

ShardClient = Client
__version__ = "0.6.6"

__all__ = [
    "Client",
    "ShardClient",
    "ShardError",
    "ConnectionError",
    "AuthenticationError",
    "InsufficientCreditsError",
    "RateLimitError",
    "MeshDegradedError",
    "InvalidRequestError",
    "ServerError",
]
