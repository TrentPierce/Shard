from __future__ import annotations

import httpx
import pytest

from shard.errors import RateLimitError


def test_wallet_balance_success(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/credits/w1"
        return httpx.Response(200, json={"wallet": "w1", "balance": 42})

    client = make_client(handler)
    bal = client.wallet.balance("w1")
    assert bal.balance == 42


def test_wallet_balance_error(make_client):
    def handler(_: httpx.Request) -> httpx.Response:
        return httpx.Response(429, text="slow down")

    client = make_client(handler)
    with pytest.raises(RateLimitError):
        client.wallet.balance("w1")


def test_wallet_tx_edge_minimal(make_client):
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/credits/tx/tx1"
        return httpx.Response(200, json={"tx": {"tx_id": "tx1"}})

    client = make_client(handler)
    tx = client.wallet.transaction("tx1")
    assert tx.tx_id == "tx1"
