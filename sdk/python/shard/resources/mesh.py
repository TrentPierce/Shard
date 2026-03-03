from __future__ import annotations

from typing import TYPE_CHECKING

from shard.models.mesh import MeshTopology, Peer

if TYPE_CHECKING:
    from shard.client import Client


class MeshResource:
    def __init__(self, client: "Client") -> None:
        self._client = client

    def topology(self) -> MeshTopology:
        """Return topology information from `/topology`."""
        response = self._client._request("GET", "/topology")
        return MeshTopology.model_validate(response.json())

    def peers(self) -> list[Peer]:
        """Return currently connected peers."""
        response = self._client._request("GET", "/peers")
        payload = response.json()
        return [Peer.model_validate(item) for item in payload.get("peers", [])]

    def bootstrap(self) -> dict:
        """Return bootstrap-discovery information for this node."""
        response = self._client._request("GET", "/v1/system/bootstrap")
        return response.json()
