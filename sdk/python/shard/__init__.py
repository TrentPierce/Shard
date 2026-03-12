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

__version__ = "0.6.5"

__all__ = [
    "Client",
    "ShardError",
    "ConnectionError",
    "AuthenticationError",
    "InsufficientCreditsError",
    "RateLimitError",
    "MeshDegradedError",
    "InvalidRequestError",
    "ServerError",
]
