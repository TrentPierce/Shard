from __future__ import annotations

import httpx

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
from shard.resources.chat import ChatResource
from shard.resources.mesh import MeshResource
from shard.resources.metrics import MetricsResource
from shard.resources.node import NodeResource
from shard.resources.wallet import WalletResource


class Client:
    """Shard API client.

    Args:
        base_url: URL of the Shard daemon.
        wallet: Optional wallet address header.
        api_key: Optional bearer API key.
        timeout: Request timeout in seconds.
    """

    def __init__(
        self,
        base_url: str = "http://localhost:9091",
        wallet: str | None = None,
        api_key: str | None = None,
        timeout: float = 30.0,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.wallet_address = wallet
        self.api_key = api_key

        headers = {"Content-Type": "application/json"}
        if wallet:
            headers["X-Shard-Wallet"] = wallet
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"

        self._http = httpx.Client(base_url=self.base_url, headers=headers, timeout=timeout)

        self.chat = ChatResource(self)
        self.node = NodeResource(self)
        self.wallet = WalletResource(self)
        self.mesh = MeshResource(self)
        self.metrics = MetricsResource(self)

    def _request(self, method: str, path: str, **kwargs) -> httpx.Response:
        try:
            response = self._http.request(method, path, **kwargs)
        except httpx.ConnectError as exc:
            raise ConnectionError(f"Cannot connect to Shard daemon at {self.base_url}: {exc}") from exc
        except httpx.TimeoutException as exc:
            raise ConnectionError(f"Request timed out: {exc}") from exc
        self._raise_for_status(response)
        return response

    def _raise_for_status(self, response: httpx.Response) -> None:
        if response.is_success:
            return
        body = response.text
        code = response.status_code
        if code in {400, 422}:
            raise InvalidRequestError(f"Invalid request: {body}", code, body)
        if code == 401:
            raise AuthenticationError(f"Authentication failed: {body}", code, body)
        if code == 402:
            raise InsufficientCreditsError(f"Insufficient credits: {body}", code, body)
        if code == 429:
            raise RateLimitError(f"Rate limit exceeded: {body}", code, body)
        if code == 503:
            raise MeshDegradedError(f"Mesh degraded: {body}", code, body)
        if code >= 500:
            raise ServerError(f"Server error {code}: {body}", code, body)
        raise ShardError(f"Unexpected HTTP {code}: {body}", code, body)

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> "Client":
        return self

    def __exit__(self, *args: object) -> None:
        self.close()
