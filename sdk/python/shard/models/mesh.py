from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class Peer(BaseModel):
    model_config = ConfigDict(extra="ignore")
    peer_id: str | None = None
    verified: bool | None = None
    avg_latency_ms: float | None = None


class MeshTopology(BaseModel):
    model_config = ConfigDict(extra="ignore")
    status: str | None = None
    shard_peer_id: str | None = None
    known_peer_count: int | None = None
    connected_peers: int | None = None
    listen_addrs: list[str] | None = None
    public_api: bool | None = None
