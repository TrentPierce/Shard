from __future__ import annotations

from typing import TYPE_CHECKING

from shard.models.wallet import Transaction, WalletAddress, WalletBalance

if TYPE_CHECKING:
    from shard.client import Client


class WalletResource:
    def __init__(self, client: "Client") -> None:
        self._client = client

    def address(self) -> WalletAddress:
        """Return this node wallet address."""
        response = self._client._request("GET", "/wallet/address")
        return WalletAddress.model_validate(response.json())

    def balance(self, wallet: str) -> WalletBalance:
        """Return wallet balance for the requested address."""
        response = self._client._request("GET", f"/credits/{wallet}")
        return WalletBalance.model_validate(response.json())

    def transaction(self, tx_id: str) -> Transaction:
        """Return a credit transaction by transaction id."""
        response = self._client._request("GET", f"/credits/tx/{tx_id}")
        payload = response.json()
        if "tx" in payload:
            return Transaction.model_validate(payload["tx"])
        return Transaction.model_validate(payload)

    def ledger_head(self) -> dict:
        """Return ledger head metadata."""
        response = self._client._request("GET", "/ledger/head")
        return response.json()

    def ledger_stats(self) -> dict:
        """Return ledger stats summary."""
        response = self._client._request("GET", "/ledger/stats")
        return response.json()
