from __future__ import annotations


class ShardError(Exception):
    """Base exception for all Shard SDK errors."""

    def __init__(
        self,
        message: str,
        status_code: int | None = None,
        response_body: str | None = None,
    ) -> None:
        super().__init__(message)
        self.status_code = status_code
        self.response_body = response_body


class ConnectionError(ShardError):
    """Raised when the SDK cannot reach the Shard daemon."""


class AuthenticationError(ShardError):
    """Raised when API key or wallet authentication fails (HTTP 401)."""


class InsufficientCreditsError(ShardError):
    """Raised when the wallet has insufficient Shard credits (HTTP 402)."""


class RateLimitError(ShardError):
    """Raised when the request rate limit is exceeded (HTTP 429)."""


class MeshDegradedError(ShardError):
    """Raised when the mesh has insufficient bootstrap connectivity (HTTP 503)."""


class InvalidRequestError(ShardError):
    """Raised when the request payload is invalid (HTTP 400/422)."""


class ServerError(ShardError):
    """Raised for unexpected server errors (HTTP 5xx)."""
