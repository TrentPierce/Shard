use super::*;
use crate::provenance::{
    build_execution_summary, new_receipt_id, CapabilityDescriptor, ExecutionModelMetadata,
    ExecutionPolicy, ExecutionReceipt, ExecutionSummary, ExecutionTaskContext, PlannerSubQuestion,
    ProvenanceGraph, ProvenanceGraphBuilder, ReceiptEventKind, ReceiptWriter,
    ResearchBriefArtifact, ResearchSourceSummary, SupplyTier, TrustTier,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

const RESEARCH_WORKFLOW_KIND: &str = "research_brief";
const MAX_RESEARCH_SOURCES: usize = 6;
const MAX_SOURCE_CHARS: usize = 6_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSource {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskRequest {
    pub workflow_kind: String,
    pub question: String,
    pub sources: Vec<ResearchSource>,
    #[serde(default)]
    pub policy: ExecutionPolicy,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskResponse {
    pub ok: bool,
    #[serde(default)]
    pub detail: Option<String>,
    pub execution: ExecutionSummary,
    pub provenance: ProvenanceGraph,
    #[serde(default)]
    pub receipts: Vec<ExecutionReceipt>,
}

#[derive(Debug, Clone)]
struct WorkflowStep {
    step_id: String,
    step_kind: String,
    prompt: String,
    required_tags: Vec<String>,
    max_tokens: u32,
}

#[derive(Debug, Clone)]
struct StepExecutionOutput {
    text: String,
    latency_ms: u64,
    model_metadata: ExecutionModelMetadata,
    actual_cost_usd: f64,
    summary: String,
}

#[derive(Debug, Clone)]
struct StepExecutionError {
    detail: String,
}

#[derive(Debug, Clone, Copy)]
struct StepExecutorContext {
    control_port: u16,
}

#[derive(Debug, Clone)]
struct StepRunResult {
    text: String,
    completed_receipt_id: String,
    actual_cost_usd: f64,
    supply_tier: SupplyTier,
}

#[derive(Debug, Clone)]
struct WorkflowExecutionError {
    detail: String,
    parent_receipt_id: String,
}

impl std::fmt::Display for WorkflowExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.detail.as_str())
    }
}

impl std::error::Error for WorkflowExecutionError {}

#[derive(Debug, Clone)]
struct ResearchWorkflowResult {
    artifact: ResearchBriefArtifact,
    total_cost_usd: f64,
    final_parent_receipt_id: String,
}

#[derive(Debug, Clone)]
struct PlannerPlan {
    planner_notes: String,
    sub_questions: Vec<PlannerSubQuestion>,
    selected_source_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlannerPlanResponse {
    #[serde(default)]
    planner_notes: Option<String>,
    #[serde(default)]
    sub_questions: Vec<PlannerSubQuestion>,
    #[serde(default)]
    selected_source_ids: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy)]
struct PolicyEvaluator;

#[async_trait]
trait StepExecutor: Send + Sync {
    async fn execute(
        &self,
        context: StepExecutorContext,
        request: &AgentTaskRequest,
        candidate: &CapabilityDescriptor,
        step: &WorkflowStep,
        policy: &ExecutionPolicy,
        remaining_deadline_ms: Option<u64>,
        request_id: &str,
    ) -> Result<StepExecutionOutput, StepExecutionError>;
}

#[derive(Clone)]
struct LocalChatStepExecutor {
    http: reqwest::Client,
}

impl LocalChatStepExecutor {
    fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl StepExecutor for LocalChatStepExecutor {
    async fn execute(
        &self,
        context: StepExecutorContext,
        request: &AgentTaskRequest,
        candidate: &CapabilityDescriptor,
        step: &WorkflowStep,
        policy: &ExecutionPolicy,
        remaining_deadline_ms: Option<u64>,
        request_id: &str,
    ) -> Result<StepExecutionOutput, StepExecutionError> {
        let base_url = candidate
            .endpoint
            .clone()
            .filter(|endpoint| !endpoint.trim().is_empty())
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", context.control_port));
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-shard-request-id"),
            reqwest::header::HeaderValue::from_str(request_id)
                .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static("agent-task")),
        );
        if candidate.supply_tier == SupplyTier::Private {
            headers.insert(
                reqwest::header::HeaderName::from_static("x-shard-route"),
                reqwest::header::HeaderValue::from_static("private"),
            );
        }

        let timeout_ms = remaining_deadline_ms
            .or(policy.deadline_ms)
            .unwrap_or(45_000)
            .clamp(1_000, 120_000);
        let started_at = SystemTime::now();
        let response = self
            .http
            .post(url.as_str())
            .headers(headers)
            .timeout(Duration::from_millis(timeout_ms))
            .json(&serde_json::json!({
                "model": request.model.clone(),
                "messages": [
                    {
                        "role": "system",
                        "content": "You are a Shard research workflow worker. Return only task output with no prefatory text."
                    },
                    {
                        "role": "user",
                        "content": step.prompt,
                    }
                ],
                "stream": false,
                "max_tokens": step.max_tokens,
            }))
            .send()
            .await
            .map_err(|error| StepExecutionError {
                detail: format!("request_failed:{error}"),
            })?;

        let latency_ms = started_at
            .elapsed()
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0);
        if !response.status().is_success() {
            return Err(StepExecutionError {
                detail: format!("upstream_status:{}", response.status()),
            });
        }

        let headers = response.headers().clone();
        let body = response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| StepExecutionError {
                detail: format!("invalid_json:{error}"),
            })?;
        let text = body
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .map(str::trim)
            .filter(|content| !content.is_empty())
            .ok_or_else(|| StepExecutionError {
                detail: "missing_completion_text".to_string(),
            })?
            .to_string();

        let prompt_tokens = estimate_token_count(step.prompt.as_str());
        let output_tokens = estimate_token_count(text.as_str());
        let actual_cost_usd = estimate_step_cost_usd(candidate, prompt_tokens, output_tokens);

        Ok(StepExecutionOutput {
            summary: preview_text(text.as_str(), 140),
            text,
            latency_ms,
            actual_cost_usd,
            model_metadata: ExecutionModelMetadata {
                model_id: request.model.clone(),
                inference_mode: Some("standard".to_string()),
                served_by: headers
                    .get("x-shard-served-by")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                mesh_forwarded: headers
                    .get("x-shard-mesh-forwarded")
                    .and_then(|value| value.to_str().ok())
                    .map(|value| value.eq_ignore_ascii_case("true")),
                mesh_forward_target: headers
                    .get("x-shard-mesh-forward-target")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                mesh_target_tier: headers
                    .get("x-shard-mesh-target-tier")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                mesh_detail: headers
                    .get("x-shard-mesh-detail")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
            },
        })
    }
}

struct ExecutionController<E> {
    state: SharedState,
    receipt_writer: ReceiptWriter,
    policy_evaluator: PolicyEvaluator,
    graph_builder: ProvenanceGraphBuilder,
    executor: E,
}

async fn execute_ranked_step<E>(
    receipt_writer: &ReceiptWriter,
    executor: &E,
    executor_context: StepExecutorContext,
    node_public_key: &str,
    execution_id: &str,
    workflow_kind: &str,
    request: &AgentTaskRequest,
    policy: &ExecutionPolicy,
    parent_receipt_id: &str,
    step: &WorkflowStep,
    rankings: Vec<CapabilityDescriptor>,
    remaining_deadline_ms: Option<u64>,
) -> Result<StepRunResult>
where
    E: StepExecutor,
{
    let ranking_receipt = ExecutionReceipt {
        receipt_id: new_receipt_id(),
        execution_id: execution_id.to_string(),
        step_id: step.step_id.clone(),
        attempt_id: format!("{execution_id}:{}:rank", step.step_id),
        parent_receipt_id: Some(parent_receipt_id.to_string()),
        event_kind: ReceiptEventKind::CandidateRanked,
        timestamp_ms: now_ms(),
        workflow_kind: workflow_kind.to_string(),
        step_kind: Some(step.step_kind.clone()),
        task_context: None,
        policy_snapshot: Some(policy.clone()),
        candidate_rankings: rankings.clone(),
        selected_candidate: rankings.first().cloned(),
        supply_tier: rankings
            .first()
            .map(|candidate| candidate.supply_tier.clone()),
        trust_tier: rankings
            .first()
            .map(|candidate| candidate.trust_tier.clone()),
        capability_match_reason: rankings
            .first()
            .and_then(|candidate| candidate.selection_reason.clone()),
        estimated_cost_usd: rankings.first().map(|candidate| {
            estimate_step_cost_usd(
                candidate,
                estimate_token_count(step.prompt.as_str()),
                step.max_tokens as usize,
            )
        }),
        actual_cost_usd: None,
        latency_ms: None,
        outcome: Some("ranked".to_string()),
        failure_reason: None,
        fallback_reason: None,
        node_identity: Some(node_public_key.to_string()),
        agent_identity: Some(step.step_kind.clone()),
        model_metadata: None,
        summary: Some(format!("Ranked {} candidate(s)", rankings.len())),
        result: None,
    };
    receipt_writer.append(&ranking_receipt).await?;

    if rankings.is_empty() {
        let failed = ExecutionReceipt {
            receipt_id: new_receipt_id(),
            execution_id: execution_id.to_string(),
            step_id: step.step_id.clone(),
            attempt_id: format!("{execution_id}:{}:none", step.step_id),
            parent_receipt_id: Some(ranking_receipt.receipt_id.clone()),
            event_kind: ReceiptEventKind::Failed,
            timestamp_ms: now_ms(),
            workflow_kind: workflow_kind.to_string(),
            step_kind: Some(step.step_kind.clone()),
            task_context: None,
            policy_snapshot: Some(policy.clone()),
            candidate_rankings: Vec::new(),
            selected_candidate: None,
            supply_tier: None,
            trust_tier: Some(policy.trust_tier.clone()),
            capability_match_reason: Some("no_candidate_satisfied_policy".to_string()),
            estimated_cost_usd: None,
            actual_cost_usd: None,
            latency_ms: None,
            outcome: Some("failed".to_string()),
            failure_reason: Some("no_candidate_satisfied_policy".to_string()),
            fallback_reason: None,
            node_identity: Some(node_public_key.to_string()),
            agent_identity: Some(step.step_kind.clone()),
            model_metadata: None,
            summary: Some("No candidate satisfied policy constraints".to_string()),
            result: None,
        };
        receipt_writer.append(&failed).await?;
        return Err(WorkflowExecutionError {
            detail: format!("no candidate satisfied policy for step {}", step.step_kind),
            parent_receipt_id: failed.receipt_id,
        }
        .into());
    }

    let mut previous_receipt_id = ranking_receipt.receipt_id.clone();
    for (index, candidate) in rankings.iter().enumerate() {
        let attempt_id = format!("{execution_id}:{}:attempt:{}", step.step_id, index + 1);
        let estimated_cost = estimate_step_cost_usd(
            candidate,
            estimate_token_count(step.prompt.as_str()),
            step.max_tokens as usize,
        );
        let dispatched = ExecutionReceipt {
            receipt_id: new_receipt_id(),
            execution_id: execution_id.to_string(),
            step_id: step.step_id.clone(),
            attempt_id: attempt_id.clone(),
            parent_receipt_id: Some(previous_receipt_id.clone()),
            event_kind: ReceiptEventKind::Dispatched,
            timestamp_ms: now_ms(),
            workflow_kind: workflow_kind.to_string(),
            step_kind: Some(step.step_kind.clone()),
            task_context: None,
            policy_snapshot: Some(policy.clone()),
            candidate_rankings: Vec::new(),
            selected_candidate: Some(candidate.clone()),
            supply_tier: Some(candidate.supply_tier.clone()),
            trust_tier: Some(candidate.trust_tier.clone()),
            capability_match_reason: candidate.selection_reason.clone(),
            estimated_cost_usd: Some(estimated_cost),
            actual_cost_usd: None,
            latency_ms: None,
            outcome: Some("dispatched".to_string()),
            failure_reason: None,
            fallback_reason: None,
            node_identity: Some(node_public_key.to_string()),
            agent_identity: Some(step.step_kind.clone()),
            model_metadata: None,
            summary: Some(format!(
                "Dispatching {} via {}",
                step.step_kind, candidate.display_name
            )),
            result: None,
        };
        receipt_writer.append(&dispatched).await?;

        let request_id = format!("{execution_id}-{}", index + 1);
        match executor
            .execute(
                executor_context,
                request,
                candidate,
                step,
                policy,
                remaining_deadline_ms,
                request_id.as_str(),
            )
            .await
        {
            Ok(output) => {
                let completed = ExecutionReceipt {
                    receipt_id: new_receipt_id(),
                    execution_id: execution_id.to_string(),
                    step_id: step.step_id.clone(),
                    attempt_id,
                    parent_receipt_id: Some(dispatched.receipt_id.clone()),
                    event_kind: ReceiptEventKind::Completed,
                    timestamp_ms: now_ms(),
                    workflow_kind: workflow_kind.to_string(),
                    step_kind: Some(step.step_kind.clone()),
                    task_context: None,
                    policy_snapshot: Some(policy.clone()),
                    candidate_rankings: Vec::new(),
                    selected_candidate: Some(candidate.clone()),
                    supply_tier: Some(candidate.supply_tier.clone()),
                    trust_tier: Some(candidate.trust_tier.clone()),
                    capability_match_reason: candidate.selection_reason.clone(),
                    estimated_cost_usd: Some(estimated_cost),
                    actual_cost_usd: Some(output.actual_cost_usd),
                    latency_ms: Some(output.latency_ms),
                    outcome: Some("completed".to_string()),
                    failure_reason: None,
                    fallback_reason: None,
                    node_identity: output
                        .model_metadata
                        .served_by
                        .clone()
                        .or_else(|| Some(node_public_key.to_string())),
                    agent_identity: Some(step.step_kind.clone()),
                    model_metadata: Some(output.model_metadata),
                    summary: Some(output.summary),
                    result: None,
                };
                receipt_writer.append(&completed).await?;
                return Ok(StepRunResult {
                    text: output.text,
                    completed_receipt_id: completed.receipt_id,
                    actual_cost_usd: output.actual_cost_usd,
                    supply_tier: candidate.supply_tier.clone(),
                });
            }
            Err(error) => {
                let failed = ExecutionReceipt {
                    receipt_id: new_receipt_id(),
                    execution_id: execution_id.to_string(),
                    step_id: step.step_id.clone(),
                    attempt_id,
                    parent_receipt_id: Some(dispatched.receipt_id.clone()),
                    event_kind: ReceiptEventKind::Failed,
                    timestamp_ms: now_ms(),
                    workflow_kind: workflow_kind.to_string(),
                    step_kind: Some(step.step_kind.clone()),
                    task_context: None,
                    policy_snapshot: Some(policy.clone()),
                    candidate_rankings: Vec::new(),
                    selected_candidate: Some(candidate.clone()),
                    supply_tier: Some(candidate.supply_tier.clone()),
                    trust_tier: Some(candidate.trust_tier.clone()),
                    capability_match_reason: candidate.selection_reason.clone(),
                    estimated_cost_usd: Some(estimated_cost),
                    actual_cost_usd: None,
                    latency_ms: None,
                    outcome: Some("failed".to_string()),
                    failure_reason: Some(error.detail.clone()),
                    fallback_reason: None,
                    node_identity: Some(node_public_key.to_string()),
                    agent_identity: Some(step.step_kind.clone()),
                    model_metadata: None,
                    summary: Some(format!("Attempt failed on {}", candidate.display_name)),
                    result: None,
                };
                receipt_writer.append(&failed).await?;
                previous_receipt_id = failed.receipt_id.clone();
                if index + 1 < rankings.len() {
                    let fallback = ExecutionReceipt {
                        receipt_id: new_receipt_id(),
                        execution_id: execution_id.to_string(),
                        step_id: step.step_id.clone(),
                        attempt_id: format!(
                            "{execution_id}:{}:fallback:{}",
                            step.step_id,
                            index + 1
                        ),
                        parent_receipt_id: Some(previous_receipt_id.clone()),
                        event_kind: ReceiptEventKind::FallbackApplied,
                        timestamp_ms: now_ms(),
                        workflow_kind: workflow_kind.to_string(),
                        step_kind: Some(step.step_kind.clone()),
                        task_context: None,
                        policy_snapshot: Some(policy.clone()),
                        candidate_rankings: Vec::new(),
                        selected_candidate: rankings.get(index + 1).cloned(),
                        supply_tier: rankings
                            .get(index + 1)
                            .map(|candidate| candidate.supply_tier.clone()),
                        trust_tier: rankings
                            .get(index + 1)
                            .map(|candidate| candidate.trust_tier.clone()),
                        capability_match_reason: Some(
                            "advancing_to_next_ranked_candidate".to_string(),
                        ),
                        estimated_cost_usd: None,
                        actual_cost_usd: None,
                        latency_ms: None,
                        outcome: Some("fallback_applied".to_string()),
                        failure_reason: None,
                        fallback_reason: Some(error.detail),
                        node_identity: Some(node_public_key.to_string()),
                        agent_identity: Some(step.step_kind.clone()),
                        model_metadata: None,
                        summary: Some("Applying ranked fallback".to_string()),
                        result: None,
                    };
                    receipt_writer.append(&fallback).await?;
                    previous_receipt_id = fallback.receipt_id;
                }
            }
        }
    }

    Err(WorkflowExecutionError {
        detail: format!("all candidates failed for step {}", step.step_kind),
        parent_receipt_id: previous_receipt_id,
    }
    .into())
}

impl<E> ExecutionController<E>
where
    E: StepExecutor,
{
    fn new(state: SharedState, receipt_writer: ReceiptWriter, executor: E) -> Self {
        Self {
            state,
            receipt_writer,
            policy_evaluator: PolicyEvaluator,
            graph_builder: ProvenanceGraphBuilder,
            executor,
        }
    }

    async fn run_research_brief(&self, request: AgentTaskRequest) -> Result<AgentTaskResponse> {
        let execution_id = format!("exec-{}", uuid::Uuid::new_v4());
        let policy = normalize_policy(request.policy.clone());
        let workflow_started_ms = now_ms();
        let task_context = ExecutionTaskContext {
            workflow_kind: request.workflow_kind.clone(),
            question: request.question.clone(),
            source_count: request.sources.len(),
            source_ids: request
                .sources
                .iter()
                .map(|source| source.id.clone())
                .collect(),
        };
        let planned = ExecutionReceipt {
            receipt_id: new_receipt_id(),
            execution_id: execution_id.clone(),
            step_id: "workflow".to_string(),
            attempt_id: format!("workflow-{}", uuid::Uuid::new_v4()),
            parent_receipt_id: None,
            event_kind: ReceiptEventKind::Planned,
            timestamp_ms: now_ms(),
            workflow_kind: request.workflow_kind.clone(),
            step_kind: Some("workflow".to_string()),
            task_context: Some(task_context),
            policy_snapshot: Some(policy.clone()),
            candidate_rankings: Vec::new(),
            selected_candidate: None,
            supply_tier: None,
            trust_tier: Some(policy.trust_tier.clone()),
            capability_match_reason: Some("workflow_received".to_string()),
            estimated_cost_usd: None,
            actual_cost_usd: None,
            latency_ms: None,
            outcome: Some("planned".to_string()),
            failure_reason: None,
            fallback_reason: None,
            node_identity: Some(self.state.node_public_key.clone()),
            agent_identity: Some("workflow-controller".to_string()),
            model_metadata: None,
            summary: Some(format!(
                "Research brief workflow accepted with {} sources",
                request.sources.len()
            )),
            result: None,
        };
        self.receipt_writer.append(&planned).await?;

        let (ok, detail) = match self
            .run_research_brief_steps(
                execution_id.as_str(),
                &request,
                &policy,
                workflow_started_ms,
                planned.receipt_id.as_str(),
            )
            .await
        {
            Ok(result) => {
                let final_receipt = ExecutionReceipt {
                    receipt_id: new_receipt_id(),
                    execution_id: execution_id.clone(),
                    step_id: "result".to_string(),
                    attempt_id: format!("result-{}", uuid::Uuid::new_v4()),
                    parent_receipt_id: Some(result.final_parent_receipt_id),
                    event_kind: ReceiptEventKind::Completed,
                    timestamp_ms: now_ms(),
                    workflow_kind: request.workflow_kind.clone(),
                    step_kind: Some("workflow_result".to_string()),
                    task_context: None,
                    policy_snapshot: Some(policy.clone()),
                    candidate_rankings: Vec::new(),
                    selected_candidate: None,
                    supply_tier: None,
                    trust_tier: Some(policy.trust_tier.clone()),
                    capability_match_reason: Some("workflow_complete".to_string()),
                    estimated_cost_usd: None,
                    actual_cost_usd: Some(result.total_cost_usd),
                    latency_ms: None,
                    outcome: Some("completed".to_string()),
                    failure_reason: None,
                    fallback_reason: None,
                    node_identity: Some(self.state.node_public_key.clone()),
                    agent_identity: Some("workflow-controller".to_string()),
                    model_metadata: None,
                    summary: Some("Research brief assembled".to_string()),
                    result: Some(result.artifact),
                };
                self.receipt_writer.append(&final_receipt).await?;
                (true, None)
            }
            Err(error) => {
                let failure = error.to_string();
                let parent_receipt_id = error
                    .downcast_ref::<WorkflowExecutionError>()
                    .map(|workflow_error| workflow_error.parent_receipt_id.clone())
                    .unwrap_or_else(|| planned.receipt_id.clone());
                let failed_receipt = ExecutionReceipt {
                    receipt_id: new_receipt_id(),
                    execution_id: execution_id.clone(),
                    step_id: "result".to_string(),
                    attempt_id: format!("result-{}", uuid::Uuid::new_v4()),
                    parent_receipt_id: Some(parent_receipt_id),
                    event_kind: ReceiptEventKind::Failed,
                    timestamp_ms: now_ms(),
                    workflow_kind: request.workflow_kind.clone(),
                    step_kind: Some("workflow_result".to_string()),
                    task_context: None,
                    policy_snapshot: Some(policy.clone()),
                    candidate_rankings: Vec::new(),
                    selected_candidate: None,
                    supply_tier: None,
                    trust_tier: Some(policy.trust_tier.clone()),
                    capability_match_reason: Some("workflow_failed".to_string()),
                    estimated_cost_usd: None,
                    actual_cost_usd: None,
                    latency_ms: None,
                    outcome: Some("failed".to_string()),
                    failure_reason: Some(failure.clone()),
                    fallback_reason: None,
                    node_identity: Some(self.state.node_public_key.clone()),
                    agent_identity: Some("workflow-controller".to_string()),
                    model_metadata: None,
                    summary: Some("Research brief failed before completion".to_string()),
                    result: None,
                };
                self.receipt_writer.append(&failed_receipt).await?;
                (false, Some(failure))
            }
        };

        let receipts = self
            .receipt_writer
            .list_for_execution(execution_id.as_str())
            .await?;
        let summary = build_execution_summary(&receipts)
            .ok_or_else(|| anyhow!("execution summary unavailable"))?;
        let graph = self.graph_builder.build(&receipts);
        Ok(AgentTaskResponse {
            ok,
            detail,
            execution: summary,
            provenance: graph,
            receipts,
        })
    }

    async fn run_research_brief_steps(
        &self,
        execution_id: &str,
        request: &AgentTaskRequest,
        policy: &ExecutionPolicy,
        workflow_started_ms: u128,
        planned_receipt_id: &str,
    ) -> Result<ResearchWorkflowResult> {
        let mut spent_cost_usd = 0.0;
        let mut spent_public_cost_usd = 0.0;

        let planner_step = WorkflowStep {
            step_id: "planner".to_string(),
            step_kind: "planner".to_string(),
            prompt: build_planner_prompt(request.question.as_str(), request.sources.as_slice()),
            required_tags: vec!["planning".to_string(), "reasoning".to_string()],
            max_tokens: 220,
        };
        let StepRunResult {
            text: planner_notes,
            completed_receipt_id: mut parent_receipt_id,
            actual_cost_usd,
            supply_tier,
        } = self
            .execute_step(
                execution_id,
                request.workflow_kind.as_str(),
                request,
                policy,
                planned_receipt_id,
                &planner_step,
                spent_cost_usd,
                spent_public_cost_usd,
                remaining_deadline_ms(workflow_started_ms, policy.deadline_ms),
            )
            .await?;
        spent_cost_usd += actual_cost_usd;
        if supply_tier == SupplyTier::Public {
            spent_public_cost_usd += actual_cost_usd;
        }

        let planner_plan = derive_planner_plan(
            request.question.as_str(),
            planner_notes.as_str(),
            request.sources.as_slice(),
        );
        let selected_sources = planner_selected_sources(&planner_plan, request.sources.as_slice());
        let mut source_summaries = Vec::new();
        let planner_parent_receipt_id = parent_receipt_id.clone();
        for source in selected_sources {
            let summary_step = WorkflowStep {
                step_id: format!("summarize-{}", source.id),
                step_kind: "summarize_source".to_string(),
                prompt: build_source_summary_prompt(
                    request.question.as_str(),
                    &planner_plan,
                    source,
                ),
                required_tags: vec!["summarization".to_string(), "low_cost".to_string()],
                max_tokens: 260,
            };
            let StepRunResult {
                text: summary_text,
                completed_receipt_id: receipt_id,
                actual_cost_usd,
                supply_tier,
            } = self
                .execute_step(
                    execution_id,
                    request.workflow_kind.as_str(),
                    request,
                    policy,
                    planner_parent_receipt_id.as_str(),
                    &summary_step,
                    spent_cost_usd,
                    spent_public_cost_usd,
                    remaining_deadline_ms(workflow_started_ms, policy.deadline_ms),
                )
                .await?;
            spent_cost_usd += actual_cost_usd;
            if supply_tier == SupplyTier::Public {
                spent_public_cost_usd += actual_cost_usd;
            }
            parent_receipt_id = receipt_id;
            source_summaries.push(ResearchSourceSummary {
                source_id: source.id.clone(),
                title: source.title.clone(),
                summary: summary_text,
            });
        }

        let synthesis_step = WorkflowStep {
            step_id: "synthesize-brief".to_string(),
            step_kind: "synthesize_brief".to_string(),
            prompt: build_synthesis_prompt(
                request.question.as_str(),
                &planner_plan,
                source_summaries.as_slice(),
            ),
            required_tags: vec![
                "synthesis".to_string(),
                "specialist".to_string(),
                "reasoning".to_string(),
            ],
            max_tokens: 420,
        };
        let StepRunResult {
            text: brief,
            completed_receipt_id: synthesis_receipt_id,
            actual_cost_usd,
            supply_tier: _supply_tier,
        } = self
            .execute_step(
                execution_id,
                request.workflow_kind.as_str(),
                request,
                policy,
                parent_receipt_id.as_str(),
                &synthesis_step,
                spent_cost_usd,
                spent_public_cost_usd,
                remaining_deadline_ms(workflow_started_ms, policy.deadline_ms),
            )
            .await?;
        spent_cost_usd += actual_cost_usd;

        Ok(ResearchWorkflowResult {
            artifact: ResearchBriefArtifact {
                brief,
                planner_notes: Some(planner_plan.planner_notes),
                sub_questions: planner_plan.sub_questions,
                selected_source_ids: planner_plan.selected_source_ids,
                source_summaries,
            },
            total_cost_usd: spent_cost_usd,
            final_parent_receipt_id: synthesis_receipt_id,
        })
    }

    async fn execute_step(
        &self,
        execution_id: &str,
        workflow_kind: &str,
        request: &AgentTaskRequest,
        policy: &ExecutionPolicy,
        parent_receipt_id: &str,
        step: &WorkflowStep,
        spent_cost_usd: f64,
        spent_public_cost_usd: f64,
        remaining_deadline_ms: Option<u64>,
    ) -> Result<StepRunResult> {
        let rankings = self.policy_evaluator.rank_candidates(
            capability_descriptors(&self.state).await,
            policy,
            step,
            remaining_budget(policy.budget_limit, spent_cost_usd),
            remaining_public_budget(policy.max_public_spend, spent_public_cost_usd),
            remaining_deadline_ms,
        );
        execute_ranked_step(
            &self.receipt_writer,
            &self.executor,
            StepExecutorContext {
                control_port: self.state.control_port.load(Ordering::Relaxed),
            },
            self.state.node_public_key.as_str(),
            execution_id,
            workflow_kind,
            request,
            policy,
            parent_receipt_id,
            step,
            rankings,
            remaining_deadline_ms,
        )
        .await
    }
}

impl PolicyEvaluator {
    fn rank_candidates(
        &self,
        candidates: Vec<CapabilityDescriptor>,
        policy: &ExecutionPolicy,
        step: &WorkflowStep,
        remaining_budget_usd: Option<f64>,
        remaining_public_budget_usd: Option<f64>,
        remaining_deadline_ms: Option<u64>,
    ) -> Vec<CapabilityDescriptor> {
        let prompt_tokens = estimate_token_count(step.prompt.as_str());
        let output_tokens = step.max_tokens as usize;
        let mut ranked = candidates
            .into_iter()
            .filter(|candidate| supply_tier_allowed(policy, &candidate.supply_tier))
            .filter(|candidate| {
                trust_tier_satisfies(
                    &candidate.trust_tier,
                    &policy.trust_tier,
                    &candidate.supply_tier,
                )
            })
            .filter(|candidate| candidate_matches_residency(candidate, policy))
            .filter(|candidate| {
                candidate_within_deadline(candidate, remaining_deadline_ms)
            })
            .filter(|candidate| {
                candidate_within_budget(
                    candidate,
                    remaining_budget_usd,
                    remaining_public_budget_usd,
                    prompt_tokens,
                    output_tokens,
                )
            })
            .map(|mut candidate| {
                let required_match_count = step
                    .required_tags
                    .iter()
                    .filter(|tag| candidate.tags.iter().any(|candidate_tag| candidate_tag == *tag))
                    .count() as f64;
                let policy_match_count = policy
                    .capability_tags
                    .iter()
                    .filter(|tag| candidate.tags.iter().any(|candidate_tag| candidate_tag == *tag))
                    .count() as f64;
                let latency_score = candidate
                    .latency_ms
                    .map(|latency_ms| 1.0 / (1.0 + latency_ms as f64 / 1500.0))
                    .unwrap_or(0.55);
                let queue_penalty = candidate.queue_depth.unwrap_or(0) as f64 * 0.05;
                let fallback_bias = policy
                    .fallback_order
                    .iter()
                    .position(|tier| tier == &candidate.supply_tier)
                    .map(|index| 1.0 - (index as f64 * 0.12))
                    .unwrap_or(0.5);
                let trust_bonus = trust_alignment_bonus(
                    &candidate.trust_tier,
                    &policy.trust_tier,
                    &candidate.supply_tier,
                );
                let specialist_bonus = if step.step_kind == "synthesize_brief"
                    && candidate
                        .tags
                        .iter()
                        .any(|tag| tag == "specialist" || tag == "synthesis")
                {
                    0.45
                } else {
                    0.0
                };
                let locality_bonus = if step.step_kind == "summarize_source"
                    && matches!(candidate.supply_tier, SupplyTier::Personal | SupplyTier::Private)
                {
                    0.35
                } else {
                    0.0
                };
                let public_penalty = if step.step_kind == "summarize_source"
                    && candidate.supply_tier == SupplyTier::Public
                {
                    0.30
                } else {
                    0.0
                };
                let health_bonus = if candidate.healthy.unwrap_or(true) {
                    0.2
                } else {
                    -0.6
                };
                let score = (required_match_count * 0.4)
                    + (policy_match_count * 0.2)
                    + latency_score
                    + fallback_bias
                    + trust_bonus
                    + specialist_bonus
                    + locality_bonus
                    + health_bonus
                    - public_penalty
                    - queue_penalty;
                candidate.score = Some(score);
                candidate.selection_reason = Some(format!(
                    "required_matches={},policy_matches={},latency_score={:.2},fallback_bias={:.2},trust_bonus={:.2},remaining_budget_usd={:.5},remaining_deadline_ms={}",
                    required_match_count,
                    policy_match_count,
                    latency_score,
                    fallback_bias,
                    trust_bonus,
                    remaining_budget_usd.unwrap_or(-1.0),
                    remaining_deadline_ms.map(|value| value.to_string()).unwrap_or_else(|| "none".to_string())
                ));
                candidate
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|left, right| {
            right
                .score
                .unwrap_or_default()
                .total_cmp(&left.score.unwrap_or_default())
                .then_with(|| left.candidate_id.cmp(&right.candidate_id))
        });
        ranked
    }
}

fn policy_is_blank(policy: &ExecutionPolicy) -> bool {
    policy.allowed_supply_tiers.is_empty()
        && matches!(policy.trust_tier, TrustTier::Local)
        && policy.budget_limit.is_none()
        && policy.deadline_ms.is_none()
        && policy.capability_tags.is_empty()
        && policy.fallback_order.is_empty()
        && policy.data_residency.is_none()
        && policy.max_public_spend.is_none()
}

fn normalize_policy(mut policy: ExecutionPolicy) -> ExecutionPolicy {
    if policy_is_blank(&policy) {
        policy.trust_tier = TrustTier::VerifiedMesh;
        policy.budget_limit = Some(1.25);
        policy.deadline_ms = Some(45_000);
        policy.capability_tags = vec![
            "planning".to_string(),
            "summarization".to_string(),
            "synthesis".to_string(),
        ];
        policy.max_public_spend = Some(0.35);
    }
    if policy.allowed_supply_tiers.is_empty() {
        policy.allowed_supply_tiers = vec![
            SupplyTier::Personal,
            SupplyTier::Private,
            SupplyTier::Public,
        ];
    }
    policy.allowed_supply_tiers.dedup();
    policy.fallback_order = policy
        .fallback_order
        .into_iter()
        .filter(|tier| {
            policy
                .allowed_supply_tiers
                .iter()
                .any(|allowed| allowed == tier)
        })
        .fold(Vec::new(), |mut tiers, tier| {
            if !tiers.iter().any(|existing| existing == &tier) {
                tiers.push(tier);
            }
            tiers
        });
    if policy.fallback_order.is_empty() {
        policy.fallback_order = policy.allowed_supply_tiers.clone();
    }
    policy
}

fn preview_text(raw: &str, max_chars: usize) -> String {
    let normalized = raw.replace(['\n', '\r'], " ");
    if normalized.len() <= max_chars {
        return normalized;
    }
    format!("{}...", &normalized[..max_chars.saturating_sub(3)])
}

fn estimate_token_count(raw: &str) -> usize {
    ((raw.len() as f64) / 4.0).ceil().max(1.0) as usize
}

fn estimate_step_cost_usd(
    candidate: &CapabilityDescriptor,
    input_tokens: usize,
    output_tokens: usize,
) -> f64 {
    let tokens = (input_tokens + output_tokens) as f64;
    let rate = match candidate.supply_tier {
        SupplyTier::Personal => 0.0000025,
        SupplyTier::Private => 0.0000035,
        SupplyTier::Public => 0.0000065,
    };
    (tokens * rate / 1000.0 * 100000.0).round() / 100000.0
}

fn derive_planner_plan(question: &str, raw: &str, sources: &[ResearchSource]) -> PlannerPlan {
    let valid_source_ids = sources
        .iter()
        .take(MAX_RESEARCH_SOURCES)
        .map(|source| source.id.as_str())
        .collect::<HashSet<_>>();
    let parsed = extract_json_object(raw)
        .and_then(|json| serde_json::from_str::<PlannerPlanResponse>(json).ok());

    let mut sub_questions = parsed
        .as_ref()
        .map(|payload| {
            payload
                .sub_questions
                .iter()
                .filter_map(|item| {
                    let item_question = item.question.trim();
                    if item_question.is_empty() {
                        return None;
                    }
                    let relevant_source_ids = item
                        .relevant_source_ids
                        .iter()
                        .map(|source_id| source_id.trim())
                        .filter(|source_id| valid_source_ids.contains(source_id))
                        .fold(Vec::new(), |mut source_ids, source_id| {
                            if !source_ids.iter().any(|existing| existing == source_id) {
                                source_ids.push(source_id.to_string());
                            }
                            source_ids
                        });
                    Some(PlannerSubQuestion {
                        question: item_question.to_string(),
                        relevant_source_ids,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut selected_source_ids = parsed
        .as_ref()
        .map(|payload| {
            payload
                .selected_source_ids
                .iter()
                .map(|source_id| source_id.trim())
                .filter(|source_id| valid_source_ids.contains(source_id))
                .fold(Vec::new(), |mut selected, source_id| {
                    if !selected.iter().any(|existing| existing == source_id) {
                        selected.push(source_id.to_string());
                    }
                    selected
                })
        })
        .unwrap_or_default();
    if selected_source_ids.is_empty() {
        for sub_question in &sub_questions {
            for source_id in &sub_question.relevant_source_ids {
                if !selected_source_ids
                    .iter()
                    .any(|existing| existing == source_id)
                {
                    selected_source_ids.push(source_id.clone());
                }
            }
        }
    }
    if selected_source_ids.is_empty() {
        selected_source_ids = sources
            .iter()
            .take(MAX_RESEARCH_SOURCES)
            .map(|source| source.id.clone())
            .collect();
    }
    if sub_questions.is_empty() {
        sub_questions.push(PlannerSubQuestion {
            question: format!("Answer the main question: {question}"),
            relevant_source_ids: selected_source_ids.clone(),
        });
    }

    let planner_notes = parsed
        .as_ref()
        .and_then(|payload| payload.planner_notes.as_ref())
        .map(|notes| notes.trim())
        .filter(|notes| !notes.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "Focus on {} source(s): {}.",
                selected_source_ids.len(),
                selected_source_ids.join(", ")
            )
        });

    PlannerPlan {
        planner_notes,
        sub_questions,
        selected_source_ids,
    }
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    raw.get(start..=end)
}

fn planner_selected_sources<'a>(
    planner_plan: &PlannerPlan,
    sources: &'a [ResearchSource],
) -> Vec<&'a ResearchSource> {
    let mut selected = Vec::new();
    for source_id in planner_plan
        .selected_source_ids
        .iter()
        .take(MAX_RESEARCH_SOURCES)
    {
        if let Some(source) = sources.iter().find(|source| source.id == *source_id) {
            selected.push(source);
        }
    }
    if selected.is_empty() {
        return sources.iter().take(MAX_RESEARCH_SOURCES).collect();
    }
    selected
}

fn planner_focus_for_source(planner_plan: &PlannerPlan, source_id: &str) -> String {
    let focus = planner_plan
        .sub_questions
        .iter()
        .filter(|item| {
            item.relevant_source_ids.is_empty()
                || item
                    .relevant_source_ids
                    .iter()
                    .any(|candidate| candidate == source_id)
        })
        .map(|item| format!("- {}", item.question))
        .collect::<Vec<_>>();
    if focus.is_empty() {
        "- Pull out the facts most relevant to the main question.".to_string()
    } else {
        focus.join("\n")
    }
}

fn build_planner_prompt(question: &str, sources: &[ResearchSource]) -> String {
    let mut prompt = String::from(
        "You are the planning step for a Shard research_brief workflow.\nReturn only valid JSON with this exact shape:\n{\"planner_notes\":\"short string\",\"selected_source_ids\":[\"source-id\"],\"sub_questions\":[{\"question\":\"string\",\"relevant_source_ids\":[\"source-id\"]}]}\nChoose 2 to 4 sub-questions. Only use the provided source IDs.\n\nQuestion:\n",
    );
    prompt.push_str(question);
    prompt.push_str("\n\nAvailable sources:\n");
    for source in sources.iter().take(MAX_RESEARCH_SOURCES) {
        prompt.push_str("- ");
        prompt.push_str(source.id.as_str());
        if let Some(title) = source.title.as_ref() {
            prompt.push_str(": ");
            prompt.push_str(title.as_str());
        }
        prompt.push('\n');
    }
    prompt.push_str(
        "\nDo not add markdown fences, commentary, or any text before or after the JSON object.",
    );
    prompt
}

fn build_source_summary_prompt(
    question: &str,
    planner_plan: &PlannerPlan,
    source: &ResearchSource,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("Question:\n");
    prompt.push_str(question);
    prompt.push_str("\n\nPlanner notes:\n");
    prompt.push_str(planner_plan.planner_notes.as_str());
    let focus = planner_focus_for_source(planner_plan, source.id.as_str());
    prompt.push_str("\n\nFocus questions for this source:\n");
    prompt.push_str(focus.as_str());
    prompt.push_str("\n\nSource ID: ");
    prompt.push_str(source.id.as_str());
    if let Some(title) = source.title.as_ref() {
        prompt.push_str("\nSource title: ");
        prompt.push_str(title.as_str());
    }
    prompt.push_str("\n\nSummarize the source for the question. Highlight the most relevant facts, risks, and contradictions.\n\nSource content:\n");
    prompt.push_str(
        source
            .content
            .chars()
            .take(MAX_SOURCE_CHARS)
            .collect::<String>()
            .as_str(),
    );
    prompt.push_str("\n\nReturn plain text only.");
    prompt
}

fn build_synthesis_prompt(
    question: &str,
    planner_plan: &PlannerPlan,
    source_summaries: &[ResearchSourceSummary],
) -> String {
    let mut prompt = String::from(
        "Write a research brief answering the question. Include: direct answer, supporting evidence, disagreements or uncertainty, and a short recommended next step.\n\nQuestion:\n",
    );
    prompt.push_str(question);
    prompt.push_str("\n\nPlanner notes:\n");
    prompt.push_str(planner_plan.planner_notes.as_str());
    prompt.push_str("\n\nSub-questions:\n");
    for sub_question in &planner_plan.sub_questions {
        prompt.push_str("- ");
        prompt.push_str(sub_question.question.as_str());
        if !sub_question.relevant_source_ids.is_empty() {
            prompt.push_str(" [");
            prompt.push_str(sub_question.relevant_source_ids.join(", ").as_str());
            prompt.push(']');
        }
        prompt.push('\n');
    }
    prompt.push_str("\n\nSource summaries:\n");
    for summary in source_summaries {
        prompt.push_str("- ");
        prompt.push_str(summary.source_id.as_str());
        if let Some(title) = summary.title.as_ref() {
            prompt.push_str(" (");
            prompt.push_str(title.as_str());
            prompt.push(')');
        }
        prompt.push_str(": ");
        prompt.push_str(summary.summary.as_str());
        prompt.push('\n');
    }
    prompt.push_str("\nReturn plain text only.");
    prompt
}

fn local_supply_tier(state: &SharedState) -> SupplyTier {
    if state.private_mode {
        SupplyTier::Private
    } else {
        SupplyTier::Personal
    }
}

fn local_trust_tier(state: &SharedState) -> TrustTier {
    match local_supply_tier(state) {
        SupplyTier::Personal => TrustTier::Local,
        SupplyTier::Private => TrustTier::VerifiedMesh,
        SupplyTier::Public => TrustTier::PublicSpecialist,
    }
}

fn supply_tier_allowed(policy: &ExecutionPolicy, tier: &SupplyTier) -> bool {
    policy
        .allowed_supply_tiers
        .iter()
        .any(|allowed| allowed == tier)
}

fn trust_tier_rank(tier: &TrustTier) -> u8 {
    match tier {
        TrustTier::Local => 4,
        TrustTier::VerifiedMesh => 1,
        TrustTier::PrivateAttested => 5,
        TrustTier::PublicSpecialist => 2,
    }
}

fn trust_tier_satisfies(
    candidate: &TrustTier,
    requested: &TrustTier,
    supply_tier: &SupplyTier,
) -> bool {
    match requested {
        TrustTier::Local => {
            matches!(candidate, TrustTier::Local) && *supply_tier == SupplyTier::Personal
        }
        TrustTier::VerifiedMesh => matches!(
            candidate,
            TrustTier::Local | TrustTier::VerifiedMesh | TrustTier::PrivateAttested
        ),
        TrustTier::PrivateAttested => matches!(candidate, TrustTier::PrivateAttested),
        TrustTier::PublicSpecialist => matches!(
            candidate,
            TrustTier::Local
                | TrustTier::VerifiedMesh
                | TrustTier::PrivateAttested
                | TrustTier::PublicSpecialist
        ),
    }
}

fn trust_alignment_bonus(
    candidate: &TrustTier,
    requested: &TrustTier,
    supply_tier: &SupplyTier,
) -> f64 {
    if !trust_tier_satisfies(candidate, requested, supply_tier) {
        return -1.5;
    }
    let delta = trust_tier_rank(candidate) as f64 - trust_tier_rank(requested) as f64;
    0.12 + (delta.max(0.0) * 0.02)
}

fn candidate_matches_residency(candidate: &CapabilityDescriptor, policy: &ExecutionPolicy) -> bool {
    let Some(required_residency) = policy.data_residency.as_ref() else {
        return true;
    };
    let required_residency = required_residency.trim().to_ascii_lowercase();
    if required_residency.is_empty() {
        return true;
    }
    let acceptable = residency_aliases(required_residency.as_str());
    if acceptable.is_empty() {
        return true;
    }
    if acceptable.iter().any(|alias| match alias.as_str() {
        "local" | "personal" => candidate.supply_tier == SupplyTier::Personal,
        "private" => candidate.supply_tier == SupplyTier::Private,
        "public" => candidate.supply_tier == SupplyTier::Public,
        _ => false,
    }) {
        return true;
    }
    candidate.tags.iter().any(|tag| {
        let normalized = tag.trim().to_ascii_lowercase();
        acceptable.iter().any(|alias| {
            normalized == format!("residency:{alias}") || normalized == format!("region:{alias}")
        })
    })
}

fn candidate_within_deadline(
    candidate: &CapabilityDescriptor,
    remaining_deadline_ms: Option<u64>,
) -> bool {
    let Some(deadline_ms) = remaining_deadline_ms else {
        return true;
    };
    predicted_candidate_latency_ms(candidate) <= deadline_ms
}

fn candidate_within_budget(
    candidate: &CapabilityDescriptor,
    remaining_budget_usd: Option<f64>,
    remaining_public_budget_usd: Option<f64>,
    input_tokens: usize,
    output_tokens: usize,
) -> bool {
    let estimated_cost = estimate_step_cost_usd(candidate, input_tokens, output_tokens);
    if let Some(limit) = remaining_budget_usd {
        if estimated_cost > limit {
            return false;
        }
    }
    if candidate.supply_tier == SupplyTier::Public {
        if let Some(limit) = remaining_public_budget_usd {
            return estimated_cost <= limit;
        }
    }
    true
}

fn predicted_candidate_latency_ms(candidate: &CapabilityDescriptor) -> u64 {
    let base_latency_ms = candidate.latency_ms.unwrap_or(match candidate.supply_tier {
        SupplyTier::Personal => 900,
        SupplyTier::Private => 1_400,
        SupplyTier::Public => 2_100,
    });
    let queue_penalty_ms = candidate.queue_depth.unwrap_or(0).saturating_mul(120);
    base_latency_ms.saturating_add(queue_penalty_ms)
}

fn remaining_budget(total_budget_usd: Option<f64>, spent_cost_usd: f64) -> Option<f64> {
    total_budget_usd.map(|budget| (budget - spent_cost_usd).max(0.0))
}

fn remaining_public_budget(max_public_spend_usd: Option<f64>, spent_cost_usd: f64) -> Option<f64> {
    max_public_spend_usd.map(|budget| (budget - spent_cost_usd).max(0.0))
}

fn remaining_deadline_ms(started_at_ms: u128, deadline_ms: Option<u64>) -> Option<u64> {
    deadline_ms.map(|budget_ms| {
        let elapsed_ms = now_ms().saturating_sub(started_at_ms);
        budget_ms.saturating_sub(elapsed_ms as u64)
    })
}

fn detected_deployment_residency_tags() -> Vec<String> {
    let Some(region) = std::env::var("FLY_REGION")
        .ok()
        .or_else(|| std::env::var("SHARD_REGION").ok())
        .or_else(|| std::env::var("AWS_REGION").ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Vec::new();
    };
    residency_aliases(region.as_str())
        .into_iter()
        .map(|alias| format!("residency:{alias}"))
        .chain(std::iter::once(format!("region:{region}")))
        .fold(Vec::new(), |mut tags, tag| {
            if !tags.iter().any(|existing| existing == &tag) {
                tags.push(tag);
            }
            tags
        })
}

fn residency_aliases(value: &str) -> Vec<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Vec::new();
    }
    let mut aliases = vec![normalized.clone()];
    let mut push_alias = |alias: &str| {
        if !aliases.iter().any(|existing| existing == alias) {
            aliases.push(alias.to_string());
        }
    };
    match normalized.as_str() {
        "local" | "personal" => push_alias("local"),
        "private" => push_alias("private"),
        "public" => push_alias("public"),
        "usa" => push_alias("us"),
        value
            if value.starts_with("us-")
                || matches!(
                    value,
                    "iad" | "lax" | "dfw" | "ord" | "mia" | "sea" | "sjc" | "atl" | "ewr"
                ) =>
        {
            push_alias("us");
            push_alias("na");
        }
        value
            if value.starts_with("eu-")
                || matches!(
                    value,
                    "lhr" | "ams" | "cdg" | "fra" | "mad" | "waw" | "arn" | "otp"
                ) =>
        {
            push_alias("eu");
        }
        value
            if value.starts_with("ap-")
                || matches!(value, "nrt" | "hkg" | "sin" | "syd" | "bom" | "maa") =>
        {
            push_alias("apac");
            push_alias("global");
        }
        _ => {}
    }
    aliases
}

fn derive_candidate_tags(
    capability_tier: Option<&str>,
    gpu_available: Option<bool>,
    supply_tier: &SupplyTier,
    residency_tags: &[String],
) -> Vec<String> {
    let mut tags = vec!["planning".to_string(), "summarization".to_string()];
    if gpu_available.unwrap_or(false)
        || capability_tier
            .map(|tier| tier.contains("gpu") || tier.contains("fast"))
            .unwrap_or(false)
    {
        tags.push("synthesis".to_string());
        tags.push("specialist".to_string());
        tags.push("reasoning".to_string());
    }
    match supply_tier {
        SupplyTier::Personal => {
            tags.push("personal_local".to_string());
            tags.push("low_cost".to_string());
            tags.push("residency:local".to_string());
        }
        SupplyTier::Private => {
            tags.push("private_mesh".to_string());
            tags.push("low_cost".to_string());
            tags.push("residency:private".to_string());
        }
        SupplyTier::Public => {
            tags.push("public_specialist".to_string());
            tags.push("residency:public".to_string());
        }
    }
    for tag in residency_tags {
        if !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.clone());
        }
    }
    tags
}

pub(crate) async fn capability_descriptors(state: &SharedState) -> Vec<CapabilityDescriptor> {
    let topology = state.topology.lock().await.clone();
    let reports = state.node_metric_reports.lock().await.clone();
    let bootstrap_registry = state.bootstrap_registry.lock().await.clone();
    let bootstrap_endpoint_by_peer = bootstrap_registry
        .values()
        .filter_map(|entry| {
            entry
                .public_api_addr
                .as_ref()
                .map(|endpoint| (entry.peer_id.clone(), endpoint.clone()))
        })
        .collect::<HashMap<_, _>>();
    let local_supply = local_supply_tier(state);
    let local_residency_tags = detected_deployment_residency_tags();
    let mut capabilities = Vec::new();
    let mut seen = HashSet::new();

    let local_id = state.node_public_key.clone();
    seen.insert(local_id.clone());
    capabilities.push(CapabilityDescriptor {
        candidate_id: local_id.clone(),
        display_name: "local-daemon".to_string(),
        supply_tier: local_supply.clone(),
        trust_tier: local_trust_tier(state),
        capability_tier: Some("local".to_string()),
        gpu_available: Some(true),
        public_api: Some(topology.is_public),
        endpoint: Some(format!(
            "http://127.0.0.1:{}",
            state.control_port.load(Ordering::Relaxed)
        )),
        queue_depth: Some(topology.load as u64),
        latency_ms: Some(state.avg_latency_ms.load(Ordering::Relaxed) as u64),
        score: None,
        tags: derive_candidate_tags(
            Some("local"),
            Some(true),
            &local_supply,
            &local_residency_tags,
        ),
        role: Some(state.node_role.clone()),
        healthy: Some(true),
        selection_reason: Some("local_control_plane".to_string()),
    });

    for (node_id, snapshot) in reports {
        if !snapshot.healthy || seen.contains(&node_id) {
            continue;
        }
        let Some(endpoint) = bootstrap_endpoint_by_peer.get(&node_id).cloned() else {
            continue;
        };
        let supply_tier = if state.private_mode {
            SupplyTier::Private
        } else {
            SupplyTier::Public
        };
        seen.insert(node_id.clone());
        capabilities.push(CapabilityDescriptor {
            candidate_id: node_id.clone(),
            display_name: node_id,
            supply_tier: supply_tier.clone(),
            trust_tier: if supply_tier == SupplyTier::Private {
                TrustTier::VerifiedMesh
            } else {
                TrustTier::PublicSpecialist
            },
            capability_tier: snapshot.capability_tier.clone(),
            gpu_available: snapshot.gpu_available,
            public_api: snapshot.public_api,
            endpoint: Some(endpoint),
            queue_depth: Some(snapshot.queue_depth),
            latency_ms: Some(snapshot.node_latency_ms),
            score: None,
            tags: derive_candidate_tags(
                snapshot.capability_tier.as_deref(),
                snapshot.gpu_available,
                &supply_tier,
                &[],
            ),
            role: Some(snapshot.role),
            healthy: Some(snapshot.healthy),
            selection_reason: None,
        });
    }

    for entry in bootstrap_registry.into_values() {
        if seen.contains(&entry.peer_id) {
            continue;
        }
        let Some(endpoint) = entry.public_api_addr.clone() else {
            continue;
        };
        let supply_tier = if state.private_mode {
            SupplyTier::Private
        } else {
            SupplyTier::Public
        };
        seen.insert(entry.peer_id.clone());
        capabilities.push(CapabilityDescriptor {
            candidate_id: entry.peer_id.clone(),
            display_name: entry.peer_id.clone(),
            supply_tier: supply_tier.clone(),
            trust_tier: if supply_tier == SupplyTier::Private {
                TrustTier::VerifiedMesh
            } else {
                TrustTier::PublicSpecialist
            },
            capability_tier: entry.capability_tier.clone(),
            gpu_available: entry.gpu_available,
            public_api: entry.public_api,
            endpoint: Some(endpoint),
            queue_depth: None,
            latency_ms: None,
            score: None,
            tags: derive_candidate_tags(
                entry.capability_tier.as_deref(),
                entry.gpu_available,
                &supply_tier,
                &[],
            ),
            role: entry.role.clone(),
            healthy: Some(true),
            selection_reason: None,
        });
    }

    capabilities
}

pub(crate) async fn capabilities_handler(
    AxumState(state): AxumState<SharedState>,
) -> Json<serde_json::Value> {
    let capabilities = capability_descriptors(&state).await;
    Json(serde_json::json!({
        "ok": true,
        "count": capabilities.len(),
        "capabilities": capabilities,
    }))
}

pub(crate) async fn agent_task_create_handler(
    AxumState(state): AxumState<SharedState>,
    Json(mut request): Json<AgentTaskRequest>,
) -> impl IntoResponse {
    request.workflow_kind = request.workflow_kind.trim().to_string();
    request.question = request.question.trim().to_string();
    if request.workflow_kind != RESEARCH_WORKFLOW_KIND {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "ok": false,
                "detail": "workflow_kind must be research_brief",
            })),
        )
            .into_response();
    }
    if request.question.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "ok": false,
                "detail": "question is required",
            })),
        )
            .into_response();
    }
    if request.sources.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "ok": false,
                "detail": "at least one source is required",
            })),
        )
            .into_response();
    }
    if request.sources.len() > MAX_RESEARCH_SOURCES {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "ok": false,
                "detail": format!("at most {} sources are supported in v1", MAX_RESEARCH_SOURCES),
            })),
        )
            .into_response();
    }

    let controller = ExecutionController::new(
        state.clone(),
        ReceiptWriter::new(state.receipt_store.clone()),
        LocalChatStepExecutor::new(),
    );
    match controller.run_research_brief(request).await {
        Ok(response) => (
            if response.ok {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            Json(serde_json::json!(response)),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "ok": false,
                "detail": error.to_string(),
            })),
        )
            .into_response(),
    }
}

pub(crate) async fn execution_summary_handler(
    AxumState(state): AxumState<SharedState>,
    AxumPath(execution_id): AxumPath<String>,
) -> impl IntoResponse {
    match state
        .receipt_store
        .receipts_for_execution(execution_id.as_str())
        .await
    {
        Ok(receipts) if receipts.is_empty() => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "ok": false,
                "detail": "execution not found",
            })),
        )
            .into_response(),
        Ok(receipts) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "execution": build_execution_summary(&receipts),
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "ok": false,
                "detail": error.to_string(),
            })),
        )
            .into_response(),
    }
}

pub(crate) async fn execution_receipts_handler(
    AxumState(state): AxumState<SharedState>,
    AxumPath(execution_id): AxumPath<String>,
) -> impl IntoResponse {
    match state
        .receipt_store
        .receipts_for_execution(execution_id.as_str())
        .await
    {
        Ok(receipts) if receipts.is_empty() => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "ok": false,
                "detail": "execution not found",
            })),
        )
            .into_response(),
        Ok(receipts) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "receipts": receipts,
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "ok": false,
                "detail": error.to_string(),
            })),
        )
            .into_response(),
    }
}

pub(crate) async fn execution_provenance_handler(
    AxumState(state): AxumState<SharedState>,
    AxumPath(execution_id): AxumPath<String>,
) -> impl IntoResponse {
    match state
        .receipt_store
        .receipts_for_execution(execution_id.as_str())
        .await
    {
        Ok(receipts) if receipts.is_empty() => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "ok": false,
                "detail": "execution not found",
            })),
        )
            .into_response(),
        Ok(receipts) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "provenance": ProvenanceGraphBuilder.build(&receipts),
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "ok": false,
                "detail": error.to_string(),
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};
    use tempfile::tempdir;

    #[derive(Clone)]
    struct ScriptedExecutor {
        outcomes: StdArc<StdMutex<VecDeque<Result<StepExecutionOutput, StepExecutionError>>>>,
    }

    impl ScriptedExecutor {
        fn new(outcomes: Vec<Result<StepExecutionOutput, StepExecutionError>>) -> Self {
            Self {
                outcomes: StdArc::new(StdMutex::new(outcomes.into())),
            }
        }
    }

    #[async_trait]
    impl StepExecutor for ScriptedExecutor {
        async fn execute(
            &self,
            _context: StepExecutorContext,
            _request: &AgentTaskRequest,
            _candidate: &CapabilityDescriptor,
            _step: &WorkflowStep,
            _policy: &ExecutionPolicy,
            _remaining_deadline_ms: Option<u64>,
            _request_id: &str,
        ) -> Result<StepExecutionOutput, StepExecutionError> {
            self.outcomes
                .lock()
                .expect("executor lock")
                .pop_front()
                .expect("scripted outcome")
        }
    }

    fn sample_request() -> AgentTaskRequest {
        AgentTaskRequest {
            workflow_kind: RESEARCH_WORKFLOW_KIND.to_string(),
            question: "What changed?".to_string(),
            sources: research_sources(),
            policy: sample_policy(),
            model: Some("meta-llama/Llama-3.2-3B-Instruct".to_string()),
        }
    }

    fn sample_policy() -> ExecutionPolicy {
        ExecutionPolicy {
            allowed_supply_tiers: vec![
                SupplyTier::Personal,
                SupplyTier::Private,
                SupplyTier::Public,
            ],
            trust_tier: TrustTier::Local,
            budget_limit: Some(0.25),
            deadline_ms: Some(2_500),
            capability_tags: vec!["reasoning".to_string()],
            fallback_order: vec![
                SupplyTier::Personal,
                SupplyTier::Private,
                SupplyTier::Public,
            ],
            data_residency: None,
            max_public_spend: Some(0.10),
        }
    }

    fn workflow_step(step_kind: &str, required_tags: &[&str]) -> WorkflowStep {
        WorkflowStep {
            step_id: step_kind.to_string(),
            step_kind: step_kind.to_string(),
            prompt: "Summarize the bundle and note risks".to_string(),
            required_tags: required_tags.iter().map(|tag| tag.to_string()).collect(),
            max_tokens: 220,
        }
    }

    fn candidate(
        id: &str,
        supply_tier: SupplyTier,
        trust_tier: TrustTier,
        latency_ms: u64,
        tags: &[&str],
    ) -> CapabilityDescriptor {
        CapabilityDescriptor {
            candidate_id: id.to_string(),
            display_name: id.to_string(),
            supply_tier,
            trust_tier,
            capability_tier: Some("gpu".to_string()),
            gpu_available: Some(true),
            public_api: Some(true),
            endpoint: None,
            queue_depth: Some(0),
            latency_ms: Some(latency_ms),
            score: None,
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            role: Some("verifier".to_string()),
            healthy: Some(true),
            selection_reason: None,
        }
    }

    fn research_sources() -> Vec<ResearchSource> {
        vec![
            ResearchSource {
                id: "s1".to_string(),
                title: Some("Operator memo".to_string()),
                content: "Local agents handle routine work.".to_string(),
            },
            ResearchSource {
                id: "s2".to_string(),
                title: Some("Cost report".to_string()),
                content: "Public specialists handled synthesis.".to_string(),
            },
        ]
    }

    #[test]
    fn policy_evaluator_keeps_personal_only_tasks_local() {
        let evaluator = PolicyEvaluator::default();
        let policy = ExecutionPolicy {
            allowed_supply_tiers: vec![SupplyTier::Personal],
            ..sample_policy()
        };
        let rankings = evaluator.rank_candidates(
            vec![
                candidate(
                    "personal",
                    SupplyTier::Personal,
                    TrustTier::Local,
                    80,
                    &["summarization", "planning"],
                ),
                candidate(
                    "public",
                    SupplyTier::Public,
                    TrustTier::PublicSpecialist,
                    40,
                    &["summarization", "planning"],
                ),
            ],
            &policy,
            &workflow_step("summarize_source", &["summarization"]),
            remaining_budget(policy.budget_limit, 0.0),
            remaining_public_budget(policy.max_public_spend, 0.0),
            policy.deadline_ms,
        );
        assert_eq!(rankings.len(), 1);
        assert_eq!(rankings[0].candidate_id, "personal");
    }

    #[test]
    fn policy_evaluator_prefers_private_before_public_when_fallback_says_so() {
        let evaluator = PolicyEvaluator::default();
        let policy = ExecutionPolicy {
            trust_tier: TrustTier::VerifiedMesh,
            fallback_order: vec![SupplyTier::Private, SupplyTier::Public],
            allowed_supply_tiers: vec![SupplyTier::Private, SupplyTier::Public],
            ..sample_policy()
        };
        let rankings = evaluator.rank_candidates(
            vec![
                candidate(
                    "public",
                    SupplyTier::Public,
                    TrustTier::PublicSpecialist,
                    90,
                    &["summarization", "reasoning"],
                ),
                candidate(
                    "private",
                    SupplyTier::Private,
                    TrustTier::VerifiedMesh,
                    120,
                    &["summarization", "reasoning"],
                ),
            ],
            &policy,
            &workflow_step("summarize_source", &["summarization"]),
            remaining_budget(policy.budget_limit, 0.0),
            remaining_public_budget(policy.max_public_spend, 0.0),
            policy.deadline_ms,
        );
        assert_eq!(
            rankings.first().map(|item| item.candidate_id.as_str()),
            Some("private")
        );
    }

    #[test]
    fn policy_evaluator_prefers_specialist_for_synthesis_when_budget_allows() {
        let evaluator = PolicyEvaluator::default();
        let policy = ExecutionPolicy {
            trust_tier: TrustTier::PublicSpecialist,
            ..sample_policy()
        };
        let rankings = evaluator.rank_candidates(
            vec![
                candidate(
                    "generic-local",
                    SupplyTier::Personal,
                    TrustTier::Local,
                    60,
                    &["reasoning"],
                ),
                candidate(
                    "specialist-public",
                    SupplyTier::Public,
                    TrustTier::PublicSpecialist,
                    110,
                    &["reasoning", "specialist", "synthesis"],
                ),
            ],
            &policy,
            &workflow_step("synthesize_brief", &["synthesis", "reasoning"]),
            remaining_budget(policy.budget_limit, 0.0),
            remaining_public_budget(policy.max_public_spend, 0.0),
            policy.deadline_ms,
        );
        assert_eq!(
            rankings.first().map(|item| item.candidate_id.as_str()),
            Some("specialist-public")
        );
    }

    #[test]
    fn policy_evaluator_blocks_public_candidate_when_public_budget_is_too_low() {
        let evaluator = PolicyEvaluator::default();
        let policy = ExecutionPolicy {
            max_public_spend: Some(0.000001),
            allowed_supply_tiers: vec![SupplyTier::Public],
            ..sample_policy()
        };
        let rankings = evaluator.rank_candidates(
            vec![candidate(
                "specialist-public",
                SupplyTier::Public,
                TrustTier::PublicSpecialist,
                110,
                &["reasoning", "specialist", "synthesis"],
            )],
            &policy,
            &workflow_step("synthesize_brief", &["synthesis", "reasoning"]),
            remaining_budget(policy.budget_limit, 0.0),
            remaining_public_budget(policy.max_public_spend, 0.0),
            policy.deadline_ms,
        );
        assert!(rankings.is_empty());
    }

    #[test]
    fn policy_evaluator_blocks_candidates_when_remaining_deadline_is_exhausted() {
        let evaluator = PolicyEvaluator::default();
        let policy = sample_policy();
        let rankings = evaluator.rank_candidates(
            vec![candidate(
                "personal",
                SupplyTier::Personal,
                TrustTier::Local,
                400,
                &["summarization", "planning"],
            )],
            &policy,
            &workflow_step("summarize_source", &["summarization"]),
            remaining_budget(policy.budget_limit, 0.0),
            remaining_public_budget(policy.max_public_spend, 0.0),
            Some(100),
        );
        assert!(rankings.is_empty());
    }

    #[test]
    fn normalize_policy_populates_defaults() {
        let normalized = normalize_policy(ExecutionPolicy::default());
        assert_eq!(
            normalized.allowed_supply_tiers,
            vec![
                SupplyTier::Personal,
                SupplyTier::Private,
                SupplyTier::Public
            ]
        );
        assert_eq!(normalized.fallback_order, normalized.allowed_supply_tiers);
        assert_eq!(normalized.trust_tier, TrustTier::VerifiedMesh);
        assert_eq!(normalized.budget_limit, Some(1.25));
        assert_eq!(normalized.deadline_ms, Some(45_000));
        assert_eq!(
            normalized.capability_tags,
            vec![
                "planning".to_string(),
                "summarization".to_string(),
                "synthesis".to_string()
            ]
        );
        assert_eq!(normalized.max_public_spend, Some(0.35));
    }

    #[test]
    fn normalize_policy_filters_fallback_to_allowed_tiers() {
        let normalized = normalize_policy(ExecutionPolicy {
            allowed_supply_tiers: vec![SupplyTier::Private, SupplyTier::Public],
            fallback_order: vec![
                SupplyTier::Personal,
                SupplyTier::Private,
                SupplyTier::Private,
                SupplyTier::Public,
            ],
            ..sample_policy()
        });
        assert_eq!(
            normalized.fallback_order,
            vec![SupplyTier::Private, SupplyTier::Public]
        );
    }

    #[test]
    fn planner_plan_uses_structured_sub_questions_to_select_sources() {
        let sources = research_sources();
        let plan = derive_planner_plan(
            "What changed?",
            r#"{"planner_notes":"Track cost and adoption.","selected_source_ids":[],"sub_questions":[{"question":"What changed in cost?","relevant_source_ids":["s2"]},{"question":"What changed in adoption?","relevant_source_ids":["s1","s2"]}]}"#,
            sources.as_slice(),
        );
        assert_eq!(plan.planner_notes, "Track cost and adoption.");
        assert_eq!(
            plan.selected_source_ids,
            vec!["s2".to_string(), "s1".to_string()]
        );
        assert_eq!(plan.sub_questions.len(), 2);
    }

    #[test]
    fn planner_plan_falls_back_to_main_question_when_output_is_unstructured() {
        let sources = research_sources();
        let plan = derive_planner_plan(
            "What changed?",
            "Look at both sources and compare them.",
            sources.as_slice(),
        );
        assert_eq!(
            plan.selected_source_ids,
            vec!["s1".to_string(), "s2".to_string()]
        );
        assert_eq!(plan.sub_questions.len(), 1);
        assert!(plan.sub_questions[0].question.contains("What changed?"));
    }

    #[test]
    fn residency_guard_requires_matching_tags_for_private_candidates() {
        let policy = ExecutionPolicy {
            data_residency: Some("us".to_string()),
            ..sample_policy()
        };
        let private_without_region = candidate(
            "private",
            SupplyTier::Private,
            TrustTier::VerifiedMesh,
            50,
            &["summarization", "private_mesh"],
        );
        assert!(!candidate_matches_residency(
            &private_without_region,
            &policy
        ));

        let private_with_region = CapabilityDescriptor {
            tags: vec![
                "summarization".to_string(),
                "private_mesh".to_string(),
                "residency:us".to_string(),
            ],
            ..private_without_region
        };
        assert!(candidate_matches_residency(&private_with_region, &policy));
    }

    #[tokio::test]
    async fn execute_ranked_step_records_policy_failure_when_no_candidates_match() {
        let temp = tempdir().expect("tempdir");
        let store = Arc::new(ReceiptStore::new(temp.path().join("receipts.db")));
        store.initialize().await.expect("receipt store init");

        let request = sample_request();
        let policy = sample_policy();
        let step = workflow_step("summarize_source", &["summarization"]);
        let error = execute_ranked_step(
            &ReceiptWriter::new(store.clone()),
            &ScriptedExecutor::new(Vec::new()),
            StepExecutorContext { control_port: 9091 },
            "node-local",
            "exec-test",
            RESEARCH_WORKFLOW_KIND,
            &request,
            &policy,
            "workflow-root",
            &step,
            Vec::new(),
            policy.deadline_ms,
        )
        .await
        .expect_err("no rankings should fail");

        assert!(error
            .to_string()
            .contains("no candidate satisfied policy for step summarize_source"));
        let receipts = store
            .receipts_for_execution("exec-test")
            .await
            .expect("load receipts");
        let events = receipts
            .iter()
            .map(|receipt| receipt.event_kind.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![ReceiptEventKind::CandidateRanked, ReceiptEventKind::Failed]
        );
    }

    #[tokio::test]
    async fn execute_ranked_step_emits_fallback_chain_before_second_attempt_succeeds() {
        let temp = tempdir().expect("tempdir");
        let store = Arc::new(ReceiptStore::new(temp.path().join("receipts.db")));
        store.initialize().await.expect("receipt store init");

        let request = sample_request();
        let policy = sample_policy();
        let step = workflow_step("summarize_source", &["summarization"]);
        let rankings = vec![
            candidate(
                "private-fast",
                SupplyTier::Private,
                TrustTier::VerifiedMesh,
                70,
                &["summarization", "reasoning"],
            ),
            candidate(
                "public-backup",
                SupplyTier::Public,
                TrustTier::PublicSpecialist,
                120,
                &["summarization", "reasoning", "specialist"],
            ),
        ];
        let result = execute_ranked_step(
            &ReceiptWriter::new(store.clone()),
            &ScriptedExecutor::new(vec![
                Err(StepExecutionError {
                    detail: "request_failed:first-hop".to_string(),
                }),
                Ok(StepExecutionOutput {
                    text: "Concise workflow summary".to_string(),
                    latency_ms: 84,
                    model_metadata: ExecutionModelMetadata {
                        model_id: Some("test-model".to_string()),
                        inference_mode: Some("standard".to_string()),
                        served_by: Some("public-backup".to_string()),
                        mesh_forwarded: Some(false),
                        mesh_forward_target: None,
                        mesh_target_tier: None,
                        mesh_detail: None,
                    },
                    actual_cost_usd: 0.019,
                    summary: "Summarized by backup candidate".to_string(),
                }),
            ]),
            StepExecutorContext { control_port: 9091 },
            "node-local",
            "exec-test",
            RESEARCH_WORKFLOW_KIND,
            &request,
            &policy,
            "workflow-root",
            &step,
            rankings,
            policy.deadline_ms,
        )
        .await
        .expect("fallback should recover");

        assert_eq!(result.text, "Concise workflow summary");
        assert_eq!(result.supply_tier, SupplyTier::Public);
        let receipts = store
            .receipts_for_execution("exec-test")
            .await
            .expect("load receipts");
        let events = receipts
            .iter()
            .map(|receipt| receipt.event_kind.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                ReceiptEventKind::CandidateRanked,
                ReceiptEventKind::Dispatched,
                ReceiptEventKind::Failed,
                ReceiptEventKind::FallbackApplied,
                ReceiptEventKind::Dispatched,
                ReceiptEventKind::Completed,
            ]
        );
        assert_eq!(
            receipts[3]
                .selected_candidate
                .as_ref()
                .map(|candidate| candidate.candidate_id.as_str()),
            Some("public-backup")
        );
        assert_eq!(
            receipts[3].fallback_reason.as_deref(),
            Some("request_failed:first-hop")
        );
    }

    #[tokio::test]
    async fn execute_ranked_step_returns_terminal_error_after_all_ranked_candidates_fail() {
        let temp = tempdir().expect("tempdir");
        let store = Arc::new(ReceiptStore::new(temp.path().join("receipts.db")));
        store.initialize().await.expect("receipt store init");

        let request = sample_request();
        let policy = sample_policy();
        let step = workflow_step("synthesize_brief", &["synthesis", "reasoning"]);
        let rankings = vec![
            candidate(
                "private-a",
                SupplyTier::Private,
                TrustTier::VerifiedMesh,
                85,
                &["synthesis", "reasoning"],
            ),
            candidate(
                "public-b",
                SupplyTier::Public,
                TrustTier::PublicSpecialist,
                95,
                &["synthesis", "reasoning", "specialist"],
            ),
        ];
        let error = execute_ranked_step(
            &ReceiptWriter::new(store.clone()),
            &ScriptedExecutor::new(vec![
                Err(StepExecutionError {
                    detail: "request_failed:private-a".to_string(),
                }),
                Err(StepExecutionError {
                    detail: "request_failed:public-b".to_string(),
                }),
            ]),
            StepExecutorContext { control_port: 9091 },
            "node-local",
            "exec-test",
            RESEARCH_WORKFLOW_KIND,
            &request,
            &policy,
            "workflow-root",
            &step,
            rankings,
            policy.deadline_ms,
        )
        .await
        .expect_err("every attempt should fail");

        assert!(error
            .to_string()
            .contains("all candidates failed for step synthesize_brief"));
        let receipts = store
            .receipts_for_execution("exec-test")
            .await
            .expect("load receipts");
        let events = receipts
            .iter()
            .map(|receipt| receipt.event_kind.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                ReceiptEventKind::CandidateRanked,
                ReceiptEventKind::Dispatched,
                ReceiptEventKind::Failed,
                ReceiptEventKind::FallbackApplied,
                ReceiptEventKind::Dispatched,
                ReceiptEventKind::Failed,
            ]
        );
        assert_eq!(
            receipts
                .last()
                .and_then(|receipt| receipt.failure_reason.as_deref()),
            Some("request_failed:public-b")
        );
    }
}
