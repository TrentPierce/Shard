use super::*;
use anyhow::Result;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum SupplyTier {
    #[default]
    Personal,
    Private,
    Public,
}

impl SupplyTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Private => "private",
            Self::Public => "public",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    #[default]
    Local,
    VerifiedMesh,
    PrivateAttested,
    PublicSpecialist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptEventKind {
    #[default]
    Planned,
    CandidateRanked,
    Dispatched,
    Completed,
    Failed,
    FallbackApplied,
    Orphaned,
}

impl ReceiptEventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::CandidateRanked => "candidate_ranked",
            Self::Dispatched => "dispatched",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::FallbackApplied => "fallback_applied",
            Self::Orphaned => "orphaned",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    #[default]
    Running,
    Completed,
    Failed,
    Orphaned,
}

impl ExecutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Orphaned => "orphaned",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionPolicy {
    #[serde(default = "default_supply_tiers")]
    pub allowed_supply_tiers: Vec<SupplyTier>,
    #[serde(default)]
    pub trust_tier: TrustTier,
    #[serde(default)]
    pub budget_limit: Option<f64>,
    #[serde(default)]
    pub deadline_ms: Option<u64>,
    #[serde(default)]
    pub capability_tags: Vec<String>,
    #[serde(default)]
    pub fallback_order: Vec<SupplyTier>,
    #[serde(default)]
    pub data_residency: Option<String>,
    #[serde(default)]
    pub max_public_spend: Option<f64>,
}

fn default_supply_tiers() -> Vec<SupplyTier> {
    vec![
        SupplyTier::Personal,
        SupplyTier::Private,
        SupplyTier::Public,
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityDescriptor {
    pub candidate_id: String,
    pub display_name: String,
    pub supply_tier: SupplyTier,
    pub trust_tier: TrustTier,
    #[serde(default)]
    pub capability_tier: Option<String>,
    #[serde(default)]
    pub gpu_available: Option<bool>,
    #[serde(default)]
    pub public_api: Option<bool>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub queue_depth: Option<u64>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub healthy: Option<bool>,
    #[serde(default)]
    pub selection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionModelMetadata {
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub inference_mode: Option<String>,
    #[serde(default)]
    pub served_by: Option<String>,
    #[serde(default)]
    pub mesh_forwarded: Option<bool>,
    #[serde(default)]
    pub mesh_forward_target: Option<String>,
    #[serde(default)]
    pub mesh_target_tier: Option<String>,
    #[serde(default)]
    pub mesh_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionTaskContext {
    pub workflow_kind: String,
    pub question: String,
    pub source_count: usize,
    #[serde(default)]
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResearchSourceSummary {
    pub source_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResearchBriefArtifact {
    pub brief: String,
    #[serde(default)]
    pub planner_notes: Option<String>,
    #[serde(default)]
    pub source_summaries: Vec<ResearchSourceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionReceipt {
    pub receipt_id: String,
    pub execution_id: String,
    pub step_id: String,
    pub attempt_id: String,
    #[serde(default)]
    pub parent_receipt_id: Option<String>,
    pub event_kind: ReceiptEventKind,
    pub timestamp_ms: u128,
    pub workflow_kind: String,
    #[serde(default)]
    pub step_kind: Option<String>,
    #[serde(default)]
    pub task_context: Option<ExecutionTaskContext>,
    #[serde(default)]
    pub policy_snapshot: Option<ExecutionPolicy>,
    #[serde(default)]
    pub candidate_rankings: Vec<CapabilityDescriptor>,
    #[serde(default)]
    pub selected_candidate: Option<CapabilityDescriptor>,
    #[serde(default)]
    pub supply_tier: Option<SupplyTier>,
    #[serde(default)]
    pub trust_tier: Option<TrustTier>,
    #[serde(default)]
    pub capability_match_reason: Option<String>,
    #[serde(default)]
    pub estimated_cost_usd: Option<f64>,
    #[serde(default)]
    pub actual_cost_usd: Option<f64>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub node_identity: Option<String>,
    #[serde(default)]
    pub agent_identity: Option<String>,
    #[serde(default)]
    pub model_metadata: Option<ExecutionModelMetadata>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub result: Option<ResearchBriefArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionSummary {
    pub execution_id: String,
    pub workflow_kind: String,
    pub status: ExecutionStatus,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
    #[serde(default)]
    pub current_step: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub source_count: usize,
    #[serde(default)]
    pub latest_summary: Option<String>,
    #[serde(default)]
    pub result: Option<ResearchBriefArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvenanceNode {
    pub receipt_id: String,
    #[serde(default)]
    pub parent_receipt_id: Option<String>,
    pub step_id: String,
    pub attempt_id: String,
    pub event_kind: ReceiptEventKind,
    pub timestamp_ms: u128,
    #[serde(default)]
    pub step_kind: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub supply_tier: Option<SupplyTier>,
    #[serde(default)]
    pub trust_tier: Option<TrustTier>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub estimated_cost_usd: Option<f64>,
    #[serde(default)]
    pub actual_cost_usd: Option<f64>,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub selected_candidate: Option<CapabilityDescriptor>,
    #[serde(default)]
    pub model_metadata: Option<ExecutionModelMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvenanceEdge {
    pub from_receipt_id: String,
    pub to_receipt_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvenanceGraph {
    pub execution_id: String,
    #[serde(default)]
    pub root_receipt_id: Option<String>,
    pub nodes: Vec<ProvenanceNode>,
    pub edges: Vec<ProvenanceEdge>,
    pub incomplete: bool,
}

#[derive(Debug, Clone)]
pub struct ReceiptStore {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ReceiptWriter {
    store: Arc<ReceiptStore>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ProvenanceGraphBuilder;

impl ReceiptStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub async fn initialize(&self) -> Result<()> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(path)?;
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS receipts (
                    receipt_id TEXT PRIMARY KEY,
                    execution_id TEXT NOT NULL,
                    step_id TEXT NOT NULL,
                    attempt_id TEXT NOT NULL,
                    parent_receipt_id TEXT,
                    event_kind TEXT NOT NULL,
                    timestamp_ms TEXT NOT NULL,
                    payload_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_receipts_execution_time
                    ON receipts(execution_id, timestamp_ms);
                CREATE INDEX IF NOT EXISTS idx_receipts_parent
                    ON receipts(parent_receipt_id);
                CREATE INDEX IF NOT EXISTS idx_receipts_attempt
                    ON receipts(attempt_id, timestamp_ms);",
            )?;
            repair_orphaned_dispatches(&conn)?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    pub async fn append_receipt(&self, receipt: &ExecutionReceipt) -> Result<()> {
        let path = self.path.clone();
        let payload = serde_json::to_string(receipt)?;
        let receipt = receipt.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(path)?;
            conn.execute(
                "INSERT INTO receipts (
                    receipt_id,
                    execution_id,
                    step_id,
                    attempt_id,
                    parent_receipt_id,
                    event_kind,
                    timestamp_ms,
                    payload_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    receipt.receipt_id,
                    receipt.execution_id,
                    receipt.step_id,
                    receipt.attempt_id,
                    receipt.parent_receipt_id,
                    receipt.event_kind.as_str(),
                    receipt.timestamp_ms.to_string(),
                    payload,
                ],
            )?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    pub async fn receipts_for_execution(
        &self,
        execution_id: &str,
    ) -> Result<Vec<ExecutionReceipt>> {
        let path = self.path.clone();
        let execution_id = execution_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<ExecutionReceipt>> {
            let conn = Connection::open(path)?;
            let mut stmt = conn.prepare(
                "SELECT payload_json
                 FROM receipts
                 WHERE execution_id = ?1
                 ORDER BY CAST(timestamp_ms AS INTEGER) ASC, receipt_id ASC",
            )?;
            let rows = stmt.query_map(params![execution_id], |row| row.get::<_, String>(0))?;
            let mut receipts = Vec::new();
            for row in rows {
                let payload = row?;
                receipts.push(serde_json::from_str::<ExecutionReceipt>(&payload)?);
            }
            Ok(receipts)
        })
        .await?
    }
}

impl ReceiptWriter {
    pub fn new(store: Arc<ReceiptStore>) -> Self {
        Self { store }
    }

    pub async fn append(&self, receipt: &ExecutionReceipt) -> Result<()> {
        self.store.append_receipt(receipt).await
    }

    pub async fn list_for_execution(&self, execution_id: &str) -> Result<Vec<ExecutionReceipt>> {
        self.store.receipts_for_execution(execution_id).await
    }
}

fn repair_orphaned_dispatches(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT payload_json
         FROM receipts
         ORDER BY CAST(timestamp_ms AS INTEGER) ASC, receipt_id ASC",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut latest_by_attempt: HashMap<(String, String), ExecutionReceipt> = HashMap::new();
    for row in rows {
        let payload = row?;
        let receipt = serde_json::from_str::<ExecutionReceipt>(&payload)?;
        latest_by_attempt.insert(
            (receipt.execution_id.clone(), receipt.attempt_id.clone()),
            receipt,
        );
    }

    for latest in latest_by_attempt.into_values() {
        if latest.event_kind != ReceiptEventKind::Dispatched {
            continue;
        }
        let orphaned = ExecutionReceipt {
            receipt_id: new_receipt_id(),
            execution_id: latest.execution_id.clone(),
            step_id: latest.step_id.clone(),
            attempt_id: latest.attempt_id.clone(),
            parent_receipt_id: Some(latest.receipt_id.clone()),
            event_kind: ReceiptEventKind::Orphaned,
            timestamp_ms: epoch_now_ms(),
            workflow_kind: latest.workflow_kind.clone(),
            step_kind: latest.step_kind.clone(),
            task_context: latest.task_context.clone(),
            policy_snapshot: latest.policy_snapshot.clone(),
            candidate_rankings: Vec::new(),
            selected_candidate: latest.selected_candidate.clone(),
            supply_tier: latest.supply_tier.clone(),
            trust_tier: latest.trust_tier.clone(),
            capability_match_reason: latest.capability_match_reason.clone(),
            estimated_cost_usd: latest.estimated_cost_usd,
            actual_cost_usd: latest.actual_cost_usd,
            latency_ms: None,
            outcome: Some("orphaned".to_string()),
            failure_reason: Some("daemon_restart_before_completion".to_string()),
            fallback_reason: None,
            node_identity: latest.node_identity.clone(),
            agent_identity: latest.agent_identity.clone(),
            model_metadata: latest.model_metadata.clone(),
            summary: Some("Attempt orphaned during daemon restart".to_string()),
            result: None,
        };
        let payload = serde_json::to_string(&orphaned)?;
        conn.execute(
            "INSERT INTO receipts (
                receipt_id,
                execution_id,
                step_id,
                attempt_id,
                parent_receipt_id,
                event_kind,
                timestamp_ms,
                payload_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                orphaned.receipt_id,
                orphaned.execution_id,
                orphaned.step_id,
                orphaned.attempt_id,
                orphaned.parent_receipt_id,
                orphaned.event_kind.as_str(),
                orphaned.timestamp_ms.to_string(),
                payload,
            ],
        )?;
    }
    Ok(())
}

fn epoch_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub fn new_receipt_id() -> String {
    format!("rcpt-{}", uuid::Uuid::new_v4())
}

pub fn build_execution_summary(receipts: &[ExecutionReceipt]) -> Option<ExecutionSummary> {
    ProvenanceGraphBuilder.summarize(receipts)
}

pub fn build_provenance_graph(receipts: &[ExecutionReceipt]) -> ProvenanceGraph {
    ProvenanceGraphBuilder.build(receipts)
}

impl ProvenanceGraphBuilder {
    pub fn summarize(&self, receipts: &[ExecutionReceipt]) -> Option<ExecutionSummary> {
        let first = receipts.first()?;
        let last = receipts.last()?;
        let task_context = receipts
            .iter()
            .find_map(|receipt| receipt.task_context.clone());
        let result = receipts
            .iter()
            .rev()
            .find_map(|receipt| receipt.result.clone());
        let latest_summary = receipts
            .iter()
            .rev()
            .find_map(|receipt| receipt.summary.clone());
        let current_step = receipts
            .iter()
            .rev()
            .find_map(|receipt| receipt.step_kind.clone());
        let status = if receipts.iter().rev().any(|receipt| {
            receipt.event_kind == ReceiptEventKind::Completed && receipt.result.is_some()
        }) {
            ExecutionStatus::Completed
        } else {
            match last.event_kind {
                ReceiptEventKind::Failed => ExecutionStatus::Failed,
                ReceiptEventKind::Orphaned => ExecutionStatus::Orphaned,
                _ => ExecutionStatus::Running,
            }
        };

        Some(ExecutionSummary {
            execution_id: first.execution_id.clone(),
            workflow_kind: first.workflow_kind.clone(),
            status,
            created_at_ms: first.timestamp_ms,
            updated_at_ms: last.timestamp_ms,
            current_step,
            question: task_context.as_ref().map(|ctx| ctx.question.clone()),
            source_count: task_context
                .as_ref()
                .map(|ctx| ctx.source_count)
                .unwrap_or(0),
            latest_summary,
            result,
        })
    }

    pub fn build(&self, receipts: &[ExecutionReceipt]) -> ProvenanceGraph {
        let mut nodes = Vec::with_capacity(receipts.len());
        let mut edges = Vec::new();
        let root_receipt_id = receipts.first().map(|receipt| receipt.receipt_id.clone());
        for receipt in receipts {
            if let Some(parent_receipt_id) = receipt.parent_receipt_id.as_ref() {
                edges.push(ProvenanceEdge {
                    from_receipt_id: parent_receipt_id.clone(),
                    to_receipt_id: receipt.receipt_id.clone(),
                });
            }
            nodes.push(ProvenanceNode {
                receipt_id: receipt.receipt_id.clone(),
                parent_receipt_id: receipt.parent_receipt_id.clone(),
                step_id: receipt.step_id.clone(),
                attempt_id: receipt.attempt_id.clone(),
                event_kind: receipt.event_kind.clone(),
                timestamp_ms: receipt.timestamp_ms,
                step_kind: receipt.step_kind.clone(),
                label: Some(format!(
                    "{} · {}",
                    receipt
                        .step_kind
                        .clone()
                        .unwrap_or_else(|| receipt.step_id.clone()),
                    receipt.event_kind.as_str()
                )),
                supply_tier: receipt.supply_tier.clone(),
                trust_tier: receipt.trust_tier.clone(),
                latency_ms: receipt.latency_ms,
                estimated_cost_usd: receipt.estimated_cost_usd,
                actual_cost_usd: receipt.actual_cost_usd,
                failure_reason: receipt.failure_reason.clone(),
                fallback_reason: receipt.fallback_reason.clone(),
                summary: receipt.summary.clone(),
                selected_candidate: receipt.selected_candidate.clone(),
                model_metadata: receipt.model_metadata.clone(),
            });
        }
        let incomplete = !receipts.iter().any(|receipt| {
            receipt.event_kind == ReceiptEventKind::Completed && receipt.result.is_some()
        });
        ProvenanceGraph {
            execution_id: receipts
                .first()
                .map(|receipt| receipt.execution_id.clone())
                .unwrap_or_default(),
            root_receipt_id,
            nodes,
            edges,
            incomplete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_policy() -> ExecutionPolicy {
        ExecutionPolicy {
            allowed_supply_tiers: vec![SupplyTier::Personal, SupplyTier::Public],
            trust_tier: TrustTier::VerifiedMesh,
            budget_limit: Some(1.25),
            deadline_ms: Some(20_000),
            capability_tags: vec!["summarization".to_string()],
            fallback_order: vec![SupplyTier::Personal, SupplyTier::Public],
            data_residency: Some("us".to_string()),
            max_public_spend: Some(0.50),
        }
    }

    fn sample_receipt(
        event_kind: ReceiptEventKind,
        parent_receipt_id: Option<&str>,
        receipt_id: &str,
        attempt_id: &str,
        timestamp_ms: u128,
    ) -> ExecutionReceipt {
        ExecutionReceipt {
            receipt_id: receipt_id.to_string(),
            execution_id: "exec-1".to_string(),
            step_id: "planner".to_string(),
            attempt_id: attempt_id.to_string(),
            parent_receipt_id: parent_receipt_id.map(str::to_string),
            event_kind,
            timestamp_ms,
            workflow_kind: "research_brief".to_string(),
            step_kind: Some("planner".to_string()),
            task_context: Some(ExecutionTaskContext {
                workflow_kind: "research_brief".to_string(),
                question: "What changed in the market?".to_string(),
                source_count: 2,
                source_ids: vec!["s1".to_string(), "s2".to_string()],
            }),
            policy_snapshot: Some(sample_policy()),
            candidate_rankings: vec![],
            selected_candidate: None,
            supply_tier: Some(SupplyTier::Personal),
            trust_tier: Some(TrustTier::Local),
            capability_match_reason: Some("local low-latency path".to_string()),
            estimated_cost_usd: Some(0.01),
            actual_cost_usd: None,
            latency_ms: Some(25),
            outcome: None,
            failure_reason: None,
            fallback_reason: None,
            node_identity: Some("node-local".to_string()),
            agent_identity: Some("planner".to_string()),
            model_metadata: None,
            summary: Some("Planner event".to_string()),
            result: None,
        }
    }

    #[test]
    fn provenance_graph_reconstructs_from_receipts() {
        let receipts = vec![
            sample_receipt(ReceiptEventKind::Planned, None, "r1", "a1", 1),
            sample_receipt(ReceiptEventKind::CandidateRanked, Some("r1"), "r2", "a1", 2),
            sample_receipt(ReceiptEventKind::Dispatched, Some("r2"), "r3", "a1", 3),
        ];
        let graph = build_provenance_graph(&receipts);
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.root_receipt_id.as_deref(), Some("r1"));
        assert!(graph.incomplete);
    }

    #[test]
    fn execution_summary_prefers_completed_result() {
        let mut completed = sample_receipt(ReceiptEventKind::Completed, Some("r3"), "r4", "a1", 4);
        completed.result = Some(ResearchBriefArtifact {
            brief: "Final brief".to_string(),
            planner_notes: Some("notes".to_string()),
            source_summaries: vec![],
        });
        let receipts = vec![
            sample_receipt(ReceiptEventKind::Planned, None, "r1", "a1", 1),
            sample_receipt(ReceiptEventKind::Dispatched, Some("r1"), "r3", "a1", 3),
            completed,
        ];
        let summary = build_execution_summary(&receipts).expect("summary");
        assert_eq!(summary.status, ExecutionStatus::Completed);
        assert_eq!(
            summary.result.as_ref().map(|item| item.brief.as_str()),
            Some("Final brief")
        );
    }

    #[test]
    fn execution_summary_stays_running_during_fallback_recovery() {
        let receipts = vec![
            sample_receipt(ReceiptEventKind::Planned, None, "r1", "a1", 1),
            sample_receipt(
                ReceiptEventKind::Failed,
                Some("r1"),
                "r2",
                "exec-1:planner:attempt:1",
                2,
            ),
            sample_receipt(
                ReceiptEventKind::FallbackApplied,
                Some("r2"),
                "r3",
                "exec-1:planner:fallback:1",
                3,
            ),
            sample_receipt(
                ReceiptEventKind::Dispatched,
                Some("r3"),
                "r4",
                "exec-1:planner:attempt:2",
                4,
            ),
        ];
        let summary = build_execution_summary(&receipts).expect("summary");
        assert_eq!(summary.status, ExecutionStatus::Running);
    }

    #[tokio::test]
    async fn receipt_store_marks_dispatched_attempts_as_orphaned_on_startup() {
        let temp = tempdir().expect("tempdir");
        let store = ReceiptStore::new(temp.path().join("receipts.db"));
        store.initialize().await.expect("initialize empty db");
        store
            .append_receipt(&sample_receipt(
                ReceiptEventKind::Dispatched,
                None,
                "dispatch-1",
                "attempt-1",
                10,
            ))
            .await
            .expect("append dispatch");

        let restarted = ReceiptStore::new(temp.path().join("receipts.db"));
        restarted.initialize().await.expect("reinitialize");
        let receipts = restarted
            .receipts_for_execution("exec-1")
            .await
            .expect("receipts");
        assert_eq!(receipts.len(), 2);
        assert_eq!(
            receipts.last().map(|item| item.event_kind.clone()),
            Some(ReceiptEventKind::Orphaned)
        );
    }

    #[tokio::test]
    async fn orphan_repair_keeps_execution_attempts_separate() {
        let temp = tempdir().expect("tempdir");
        let store = ReceiptStore::new(temp.path().join("receipts.db"));
        store.initialize().await.expect("initialize empty db");

        let mut first = sample_receipt(
            ReceiptEventKind::Dispatched,
            None,
            "dispatch-1",
            "planner-1",
            10,
        );
        first.execution_id = "exec-1".to_string();
        let mut second = sample_receipt(
            ReceiptEventKind::Dispatched,
            None,
            "dispatch-2",
            "planner-1",
            11,
        );
        second.execution_id = "exec-2".to_string();

        store.append_receipt(&first).await.expect("append first");
        store.append_receipt(&second).await.expect("append second");

        let restarted = ReceiptStore::new(temp.path().join("receipts.db"));
        restarted.initialize().await.expect("reinitialize");
        let first_receipts = restarted
            .receipts_for_execution("exec-1")
            .await
            .expect("exec-1 receipts");
        let second_receipts = restarted
            .receipts_for_execution("exec-2")
            .await
            .expect("exec-2 receipts");

        assert_eq!(first_receipts.len(), 2);
        assert_eq!(second_receipts.len(), 2);
        assert_eq!(
            first_receipts.last().map(|item| item.event_kind.clone()),
            Some(ReceiptEventKind::Orphaned)
        );
        assert_eq!(
            second_receipts.last().map(|item| item.event_kind.clone()),
            Some(ReceiptEventKind::Orphaned)
        );
    }
}
