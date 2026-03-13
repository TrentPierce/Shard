from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class ResearchSource(BaseModel):
    model_config = ConfigDict(extra="ignore")
    id: str
    title: str | None = None
    content: str


class ExecutionPolicy(BaseModel):
    model_config = ConfigDict(extra="ignore")
    allowed_supply_tiers: list[str] = ["personal", "private", "public"]
    trust_tier: str = "verified_mesh"
    budget_limit: float | None = 1.25
    deadline_ms: int | None = 45_000
    capability_tags: list[str] = ["planning", "summarization", "synthesis"]
    fallback_order: list[str] = ["personal", "private", "public"]
    data_residency: str | None = None
    max_public_spend: float | None = 0.35


class CapabilityDescriptor(BaseModel):
    model_config = ConfigDict(extra="ignore")
    candidate_id: str
    display_name: str
    supply_tier: str
    trust_tier: str
    capability_tier: str | None = None
    gpu_available: bool | None = None
    public_api: bool | None = None
    endpoint: str | None = None
    queue_depth: int | None = None
    latency_ms: int | None = None
    score: float | None = None
    tags: list[str] = []
    role: str | None = None
    healthy: bool | None = None
    selection_reason: str | None = None


class ExecutionModelMetadata(BaseModel):
    model_config = ConfigDict(extra="ignore")
    model_id: str | None = None
    inference_mode: str | None = None
    served_by: str | None = None
    mesh_forwarded: bool | None = None
    mesh_forward_target: str | None = None
    mesh_target_tier: str | None = None
    mesh_detail: str | None = None


class ResearchSourceSummary(BaseModel):
    model_config = ConfigDict(extra="ignore")
    source_id: str
    title: str | None = None
    summary: str


class PlannerSubQuestion(BaseModel):
    model_config = ConfigDict(extra="ignore")
    question: str
    relevant_source_ids: list[str] = []


class ResearchBriefArtifact(BaseModel):
    model_config = ConfigDict(extra="ignore")
    brief: str
    planner_notes: str | None = None
    sub_questions: list[PlannerSubQuestion] = []
    selected_source_ids: list[str] = []
    source_summaries: list[ResearchSourceSummary] = []


class ExecutionTaskContext(BaseModel):
    model_config = ConfigDict(extra="ignore")
    workflow_kind: str
    question: str
    source_count: int
    source_ids: list[str] = []


class ExecutionReceipt(BaseModel):
    model_config = ConfigDict(extra="ignore")
    receipt_id: str
    execution_id: str
    step_id: str
    attempt_id: str
    parent_receipt_id: str | None = None
    event_kind: str
    timestamp_ms: int
    workflow_kind: str
    step_kind: str | None = None
    task_context: ExecutionTaskContext | None = None
    policy_snapshot: ExecutionPolicy | None = None
    candidate_rankings: list[CapabilityDescriptor] = []
    selected_candidate: CapabilityDescriptor | None = None
    supply_tier: str | None = None
    trust_tier: str | None = None
    capability_match_reason: str | None = None
    estimated_cost_usd: float | None = None
    actual_cost_usd: float | None = None
    latency_ms: int | None = None
    outcome: str | None = None
    failure_reason: str | None = None
    fallback_reason: str | None = None
    node_identity: str | None = None
    agent_identity: str | None = None
    model_metadata: ExecutionModelMetadata | None = None
    summary: str | None = None
    result: ResearchBriefArtifact | None = None


class ExecutionSummary(BaseModel):
    model_config = ConfigDict(extra="ignore")
    execution_id: str
    workflow_kind: str
    status: str
    created_at_ms: int
    updated_at_ms: int
    current_step: str | None = None
    question: str | None = None
    source_count: int = 0
    latest_summary: str | None = None
    result: ResearchBriefArtifact | None = None


class ProvenanceNode(BaseModel):
    model_config = ConfigDict(extra="ignore")
    receipt_id: str
    parent_receipt_id: str | None = None
    step_id: str
    attempt_id: str
    event_kind: str
    timestamp_ms: int
    step_kind: str | None = None
    label: str | None = None
    supply_tier: str | None = None
    trust_tier: str | None = None
    latency_ms: int | None = None
    estimated_cost_usd: float | None = None
    actual_cost_usd: float | None = None
    failure_reason: str | None = None
    fallback_reason: str | None = None
    summary: str | None = None
    selected_candidate: CapabilityDescriptor | None = None
    model_metadata: ExecutionModelMetadata | None = None


class ProvenanceEdge(BaseModel):
    model_config = ConfigDict(extra="ignore")
    from_receipt_id: str
    to_receipt_id: str


class ProvenanceGraph(BaseModel):
    model_config = ConfigDict(extra="ignore")
    execution_id: str
    root_receipt_id: str | None = None
    nodes: list[ProvenanceNode] = []
    edges: list[ProvenanceEdge] = []
    incomplete: bool = False


class AgentTaskRequest(BaseModel):
    model_config = ConfigDict(extra="ignore")
    workflow_kind: str = "research_brief"
    question: str
    sources: list[ResearchSource]
    policy: ExecutionPolicy = ExecutionPolicy()
    model: str | None = None


class AgentTaskResponse(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool = True
    detail: str | None = None
    execution: ExecutionSummary
    provenance: ProvenanceGraph
    receipts: list[ExecutionReceipt] = []


class ExecutionStatusEnvelope(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool = True
    execution: ExecutionSummary | None = None


class ExecutionReceiptsEnvelope(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool = True
    receipts: list[ExecutionReceipt] = []


class ExecutionProvenanceEnvelope(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool = True
    provenance: ProvenanceGraph


class CapabilitiesEnvelope(BaseModel):
    model_config = ConfigDict(extra="ignore")
    ok: bool = True
    count: int = 0
    capabilities: list[CapabilityDescriptor] = []
