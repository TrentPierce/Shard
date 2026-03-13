from __future__ import annotations

from typing import TYPE_CHECKING

from shard.models.agents import (
    AgentTaskRequest,
    AgentTaskResponse,
    CapabilitiesEnvelope,
    CapabilityDescriptor,
    ExecutionPolicy,
    ExecutionProvenanceEnvelope,
    ExecutionReceipt,
    ExecutionReceiptsEnvelope,
    ExecutionStatusEnvelope,
    ExecutionSummary,
    ProvenanceGraph,
    ResearchSource,
)

if TYPE_CHECKING:
    from shard.client import Client


class AgentsResource:
    def __init__(self, client: "Client") -> None:
        self._client = client

    def submit(
        self,
        question: str,
        sources: list[dict] | list[ResearchSource],
        policy: ExecutionPolicy | dict | None = None,
        model: str | None = None,
    ) -> AgentTaskResponse:
        """Submit the v1 `research_brief` workflow."""
        policy_value = (
            ExecutionPolicy()
            if policy is None
            else policy
            if isinstance(policy, ExecutionPolicy)
            else ExecutionPolicy.model_validate(policy)
        )
        payload = AgentTaskRequest(
            question=question,
            sources=[
                source
                if isinstance(source, ResearchSource)
                else ResearchSource.model_validate(source)
                for source in sources
            ],
            policy=policy_value,
            model=model,
        )
        response = self._client._request(
            "POST",
            "/v1/agents/tasks",
            json=payload.model_dump(exclude_none=True),
        )
        return AgentTaskResponse.model_validate(response.json())

    def status(self, execution_id: str) -> ExecutionSummary | None:
        """Return the latest execution summary for a workflow."""
        response = self._client._request("GET", f"/v1/executions/{execution_id}")
        payload = ExecutionStatusEnvelope.model_validate(response.json())
        return payload.execution

    def receipts(self, execution_id: str) -> list[ExecutionReceipt]:
        """Return append-only receipts for a workflow."""
        response = self._client._request("GET", f"/v1/executions/{execution_id}/receipts")
        payload = ExecutionReceiptsEnvelope.model_validate(response.json())
        return payload.receipts

    def provenance(self, execution_id: str) -> ProvenanceGraph:
        """Return the reconstructable provenance graph for a workflow."""
        response = self._client._request("GET", f"/v1/executions/{execution_id}/provenance")
        payload = ExecutionProvenanceEnvelope.model_validate(response.json())
        return payload.provenance

    def capabilities(self) -> list[CapabilityDescriptor]:
        """Return scheduler capability descriptors visible to the daemon."""
        response = self._client._request("GET", "/v1/capabilities")
        payload = CapabilitiesEnvelope.model_validate(response.json())
        return payload.capabilities
