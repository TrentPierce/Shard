from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class ContributionAck(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool
    detail: str | None = None

