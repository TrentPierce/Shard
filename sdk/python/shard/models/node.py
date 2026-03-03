from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class LogEntry(BaseModel):
    model_config = ConfigDict(extra="ignore")
    message: str


class NodeStatus(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool | None = None
    status: str | None = None
    health_status: str | None = None
    readiness_reason: str | None = None
    peer_id: str | None = None
    connected_peers: int | None = None


class ConsensusRole(BaseModel):
    model_config = ConfigDict(extra="ignore")
    role: str
    term: int
    leader_id: str | None = None
