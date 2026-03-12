from __future__ import annotations

import httpx
import pytest

from shard import ShardClient
from shard.client import Client
from shard.errors import (
    ConnectionError,
    InsufficientCreditsError,
    InvalidRequestError,
    ShardError,
)


def _response(status: int, text: str = "err") -> httpx.Response:
    req = httpx.Request("GET", "http://testserver/any")
    return httpx.Response(status, text=text, request=req)


def test_client_headers_and_context_manager() -> None:
    with Client(base_url="http://testserver", wallet="w1", api_key="k1") as client:
        assert client._http.headers["X-Shard-Wallet"] == "w1"
        assert client._http.headers["Authorization"] == "Bearer k1"


def test_public_shard_client_alias_points_to_client() -> None:
    client = ShardClient(base_url="http://testserver")
    assert isinstance(client, Client)
    client.close()


def test_raise_for_status_additional_branches() -> None:
    client = Client(base_url="http://testserver")
    with pytest.raises(InsufficientCreditsError):
        client._raise_for_status(_response(402, "no credits"))
    with pytest.raises(InvalidRequestError):
        client._raise_for_status(_response(422, "bad schema"))
    with pytest.raises(ShardError):
        client._raise_for_status(_response(418, "teapot"))
    client.close()


def test_request_wraps_connection_and_timeout_errors(monkeypatch: pytest.MonkeyPatch) -> None:
    client = Client(base_url="http://testserver")

    def raise_connect(*_args, **_kwargs):
        raise httpx.ConnectError("offline", request=httpx.Request("GET", "http://testserver/x"))

    monkeypatch.setattr(client._http, "request", raise_connect)
    with pytest.raises(ConnectionError):
        client._request("GET", "/x")

    def raise_timeout(*_args, **_kwargs):
        raise httpx.ReadTimeout("slow", request=httpx.Request("GET", "http://testserver/x"))

    monkeypatch.setattr(client._http, "request", raise_timeout)
    with pytest.raises(ConnectionError):
        client._request("GET", "/x")
    client.close()
