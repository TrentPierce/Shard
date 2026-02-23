"""Custom exception hierarchy for the Shard SDK."""


class ShardError(Exception):
    """Base exception for all Shard SDK errors."""

    pass


class ShardAPIError(ShardError):
    """Raised when the Shard API returns an error response."""

    def __init__(
        self,
        message: str,
        status_code: int | None = None,
        response_body: str | None = None,
    ):
        super().__init__(message)
        self.status_code = status_code
        self.response_body = response_body

    def __str__(self) -> str:
        parts = [super().__str__()]
        if self.status_code:
            parts.append(f"(HTTP {self.status_code})")
        return " ".join(parts)


class ShardTimeoutError(ShardError):
    """Raised when a request times out."""

    pass


class ShardConnectionError(ShardError):
    """Raised when the SDK cannot connect to the Shard daemon."""

    pass


class ShardAuthError(ShardError):
    """Raised when authentication fails (missing or invalid API key)."""

    pass
