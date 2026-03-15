import { authHeaders, fetchWithLocalFallback } from "@/lib/api"
import { DEFAULT_MODEL_ID } from "@/lib/model"

export type SupplyTier = "personal" | "private" | "public"
export type TrustTier =
  | "local"
  | "verified_mesh"
  | "private_attested"
  | "public_specialist"
export type ReceiptEventKind =
  | "planned"
  | "candidate_ranked"
  | "dispatched"
  | "completed"
  | "failed"
  | "fallback_applied"
  | "orphaned"
export type ExecutionStatus = "running" | "completed" | "failed" | "orphaned"

export interface ExecutionPolicy {
  allowed_supply_tiers: SupplyTier[]
  trust_tier: TrustTier
  budget_limit?: number | null
  deadline_ms?: number | null
  capability_tags: string[]
  fallback_order: SupplyTier[]
  data_residency?: string | null
  max_public_spend?: number | null
}

export interface ResearchSourceInput {
  id: string
  title?: string | null
  content: string
}

export type ResearchSource = ResearchSourceInput

export interface CapabilityDescriptor {
  candidate_id: string
  display_name: string
  supply_tier: SupplyTier
  trust_tier: TrustTier
  capability_tier?: string | null
  gpu_available?: boolean | null
  public_api?: boolean | null
  endpoint?: string | null
  queue_depth?: number | null
  latency_ms?: number | null
  score?: number | null
  tags: string[]
  role?: string | null
  healthy?: boolean | null
  selection_reason?: string | null
}

export interface ExecutionModelMetadata {
  model_id?: string | null
  inference_mode?: string | null
  served_by?: string | null
  mesh_forwarded?: boolean | null
  mesh_forward_target?: string | null
  mesh_target_tier?: string | null
  mesh_detail?: string | null
  overflow_routed?: boolean | null
  overflow_destination?: string | null
}

export interface ResearchSourceSummary {
  source_id: string
  title?: string | null
  summary: string
}

export interface PlannerSubQuestion {
  question: string
  relevant_source_ids: string[]
}

export interface ResearchBriefArtifact {
  brief: string
  planner_notes?: string | null
  sub_questions?: PlannerSubQuestion[] | null
  selected_source_ids?: string[] | null
  source_summaries: ResearchSourceSummary[]
}

export interface ExecutionTaskContext {
  workflow_kind: string
  question: string
  source_count: number
  source_ids: string[]
}

export interface ExecutionReceipt {
  receipt_id: string
  execution_id: string
  step_id: string
  attempt_id: string
  parent_receipt_id?: string | null
  event_kind: ReceiptEventKind
  timestamp_ms: number
  workflow_kind: string
  step_kind?: string | null
  task_context?: ExecutionTaskContext | null
  policy_snapshot?: ExecutionPolicy | null
  candidate_rankings: CapabilityDescriptor[]
  selected_candidate?: CapabilityDescriptor | null
  supply_tier?: SupplyTier | null
  trust_tier?: TrustTier | null
  capability_match_reason?: string | null
  estimated_cost_usd?: number | null
  actual_cost_usd?: number | null
  latency_ms?: number | null
  outcome?: string | null
  failure_reason?: string | null
  fallback_reason?: string | null
  node_identity?: string | null
  agent_identity?: string | null
  model_metadata?: ExecutionModelMetadata | null
  summary?: string | null
  result?: ResearchBriefArtifact | null
}

export interface ExecutionSummary {
  execution_id: string
  workflow_kind: string
  status: ExecutionStatus
  created_at_ms: number
  updated_at_ms: number
  current_step?: string | null
  question?: string | null
  source_count: number
  latest_summary?: string | null
  result?: ResearchBriefArtifact | null
}

export interface ProvenanceNode {
  receipt_id: string
  parent_receipt_id?: string | null
  step_id: string
  attempt_id: string
  event_kind: ReceiptEventKind
  timestamp_ms: number
  step_kind?: string | null
  label?: string | null
  supply_tier?: SupplyTier | null
  trust_tier?: TrustTier | null
  latency_ms?: number | null
  estimated_cost_usd?: number | null
  actual_cost_usd?: number | null
  failure_reason?: string | null
  fallback_reason?: string | null
  summary?: string | null
  selected_candidate?: CapabilityDescriptor | null
  model_metadata?: ExecutionModelMetadata | null
}

export interface ProvenanceEdge {
  from_receipt_id: string
  to_receipt_id: string
}

export interface ProvenanceGraph {
  execution_id: string
  root_receipt_id?: string | null
  nodes: ProvenanceNode[]
  edges: ProvenanceEdge[]
  incomplete: boolean
}

export interface AgentTaskResponse {
  ok: boolean
  detail?: string | null
  execution: ExecutionSummary
  provenance: ProvenanceGraph
  receipts: ExecutionReceipt[]
}

export interface ExecutionSummaryResponse {
  ok: boolean
  execution: ExecutionSummary | null
}

export interface ExecutionReceiptsResponse {
  ok: boolean
  receipts: ExecutionReceipt[]
}

export interface ExecutionProvenanceResponse {
  ok: boolean
  provenance: ProvenanceGraph
}

export interface CapabilitiesResponse {
  ok: boolean
  count: number
  capabilities: CapabilityDescriptor[]
}

export interface ResearchBriefTaskInput {
  question: string
  sources: ResearchSourceInput[]
  policy?: Partial<ExecutionPolicy>
  model?: string
}

export interface AgentTaskRequest {
  workflow_kind: "research_brief"
  question: string
  sources: ResearchSourceInput[]
  policy: ExecutionPolicy
  model?: string
}

export function defaultExecutionPolicy(): ExecutionPolicy {
  return {
    allowed_supply_tiers: ["personal", "private", "public"],
    trust_tier: "verified_mesh",
    budget_limit: 1.25,
    deadline_ms: 45_000,
    capability_tags: ["planning", "summarization", "synthesis"],
    fallback_order: ["personal", "private", "public"],
    data_residency: null,
    max_public_spend: 0.35,
  }
}

export function createDefaultSources(): ResearchSourceInput[] {
  return [
    {
      id: "source-market-overview",
      title: "Market overview memo",
      content:
        "Shard is positioning itself as a policy-aware runtime for research agents. Teams care about latency, cost, trust tier, and failure visibility. Competitors emphasize commodity compute but rarely expose routing evidence.",
    },
    {
      id: "source-operator-feedback",
      title: "Operator interview notes",
      content:
        "Operators want concrete incentives. They are willing to contribute specialist capacity when it improves their own workflows first and gives them access to higher-value public tasks second.",
    },
  ]
}

function mergePolicy(policy?: Partial<ExecutionPolicy>): ExecutionPolicy {
  const defaults = defaultExecutionPolicy()
  const allowedSupplyTiers =
    policy?.allowed_supply_tiers && policy.allowed_supply_tiers.length > 0
      ? policy.allowed_supply_tiers
      : defaults.allowed_supply_tiers
  const capabilityTags =
    policy?.capability_tags && policy.capability_tags.length > 0
      ? policy.capability_tags
      : defaults.capability_tags
  const requestedFallbackOrder =
    policy?.fallback_order && policy.fallback_order.length > 0
      ? policy.fallback_order
      : allowedSupplyTiers
  const fallbackOrder = requestedFallbackOrder.filter((tier) =>
    allowedSupplyTiers.includes(tier),
  )

  return {
    ...defaults,
    ...policy,
    allowed_supply_tiers: allowedSupplyTiers,
    capability_tags: capabilityTags,
    fallback_order: fallbackOrder.length > 0 ? fallbackOrder : allowedSupplyTiers,
  }
}

async function parseJsonResponse<T>(path: string, init: RequestInit): Promise<T> {
  const response = await fetchWithLocalFallback(path, init)
  const payload = await response.json().catch(() => ({}))
  if (!response.ok) {
    const detail = typeof payload?.detail === "string" ? payload.detail : `${response.status} ${response.statusText}`
    throw new Error(detail)
  }
  return payload as T
}

export async function submitResearchBriefTask(
  input: ResearchBriefTaskInput,
): Promise<AgentTaskResponse> {
  return parseJsonResponse<AgentTaskResponse>("/v1/agents/tasks", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
    },
    body: JSON.stringify({
      workflow_kind: "research_brief",
      question: input.question,
      sources: input.sources,
      policy: mergePolicy(input.policy),
      model: input.model ?? DEFAULT_MODEL_ID,
    }),
  })
}

export async function fetchExecutionSummary(
  executionId: string,
): Promise<ExecutionSummaryResponse> {
  return parseJsonResponse<ExecutionSummaryResponse>(`/v1/executions/${executionId}`, {
    method: "GET",
    headers: authHeaders(),
  })
}

export async function fetchExecutionReceipts(
  executionId: string,
): Promise<ExecutionReceiptsResponse> {
  return parseJsonResponse<ExecutionReceiptsResponse>(
    `/v1/executions/${executionId}/receipts`,
    {
      method: "GET",
      headers: authHeaders(),
    },
  )
}

export async function fetchExecutionProvenance(
  executionId: string,
): Promise<ExecutionProvenanceResponse> {
  return parseJsonResponse<ExecutionProvenanceResponse>(
    `/v1/executions/${executionId}/provenance`,
    {
      method: "GET",
      headers: authHeaders(),
    },
  )
}

export async function fetchCapabilities(): Promise<CapabilitiesResponse> {
  return parseJsonResponse<CapabilitiesResponse>("/v1/capabilities", {
    method: "GET",
    headers: authHeaders(),
  })
}

function deriveExecutionDetail(
  execution: ExecutionSummary,
  receipts: ExecutionReceipt[],
): string | null {
  if (execution.status !== "failed" && execution.status !== "orphaned") {
    return null
  }
  const terminalReceipt = [...receipts].reverse().find((receipt) => {
    return receipt.failure_reason || receipt.fallback_reason || receipt.summary
  })
  return (
    terminalReceipt?.failure_reason ??
    terminalReceipt?.fallback_reason ??
    terminalReceipt?.summary ??
    null
  )
}

export async function submitResearchBrief(
  input: AgentTaskRequest,
): Promise<AgentTaskResponse> {
  return parseJsonResponse<AgentTaskResponse>("/v1/agents/tasks", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(),
    },
    body: JSON.stringify(input),
  })
}

export async function getExecutionSummary(
  executionId: string,
): Promise<ExecutionSummary | null> {
  const response = await fetchExecutionSummary(executionId)
  return response.execution
}

export async function getExecutionReceipts(
  executionId: string,
): Promise<ExecutionReceipt[]> {
  const response = await fetchExecutionReceipts(executionId)
  return response.receipts
}

export async function getExecutionProvenance(
  executionId: string,
): Promise<ProvenanceGraph> {
  const response = await fetchExecutionProvenance(executionId)
  return response.provenance
}

export async function listCapabilities(): Promise<CapabilityDescriptor[]> {
  const response = await fetchCapabilities()
  return response.capabilities
}

export async function fetchExecutionBundle(
  executionId: string,
): Promise<AgentTaskResponse> {
  const [summaryResponse, receiptsResponse, provenanceResponse] = await Promise.all([
    fetchExecutionSummary(executionId),
    fetchExecutionReceipts(executionId),
    fetchExecutionProvenance(executionId),
  ])
  const execution = summaryResponse.execution
  if (!execution) {
    throw new Error(`Execution ${executionId} was not found`)
  }
  return {
    ok: execution.status !== "failed" && execution.status !== "orphaned",
    detail: deriveExecutionDetail(execution, receiptsResponse.receipts),
    execution,
    receipts: receiptsResponse.receipts,
    provenance: provenanceResponse.provenance,
  }
}
