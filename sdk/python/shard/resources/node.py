from __future__ import annotations

from typing import TYPE_CHECKING

from shard.models.node import ConsensusRole, NodeStatus

if TYPE_CHECKING:
    from shard.client import Client


class NodeResource:
    def __init__(self, client: "Client") -> None:
        self._client = client

    def status(self) -> NodeStatus:
        """Return node status from `/node/status`."""
        response = self._client._request("GET", "/node/status")
        return NodeStatus.model_validate(response.json())

    def health(self) -> NodeStatus:
        """Return service health from `/health`."""
        response = self._client._request("GET", "/health")
        return NodeStatus.model_validate(response.json())

    def logs(self) -> list[str]:
        """Return recent daemon logs from `/node/logs`."""
        response = self._client._request("GET", "/node/logs")
        payload = response.json()
        return payload.get("logs", [])

    def set_participation(self, enabled: bool) -> dict:
        """Enable/disable contribution mode for this node."""
        response = self._client._request(
            "POST",
            "/node/toggle-participation",
            json={"enabled": enabled},
        )
        return response.json()

    def consensus_role(self) -> ConsensusRole:
        """Return current consensus role for HA mode."""
        response = self._client._request("GET", "/node/consensus-role")
        return ConsensusRole.model_validate(response.json())
