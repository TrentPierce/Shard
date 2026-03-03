from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class WalletAddress(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool | None = None
    wallet: str


class WalletBalance(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool | None = None
    wallet: str
    balance: int | float


class Transaction(BaseModel):
    model_config = ConfigDict(extra="ignore")
    tx_id: str | None = None
    request_id: str | None = None
    amount: int | float | None = None
    to_wallet: str | None = None
    from_wallet: str | None = None
