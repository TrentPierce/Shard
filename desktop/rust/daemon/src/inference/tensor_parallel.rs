use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

pub const TENSOR_PARALLEL_PROTOCOL: &str = "/shard/tensor-parallel/1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorParallelConfig {
    pub enabled: bool,
    pub degree: usize,
    pub co_verifier_timeout_ms: u64,
}

impl Default for TensorParallelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            degree: 2,
            co_verifier_timeout_ms: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub request_id: String,
    pub prompt_context: String,
    pub draft_tokens: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub accepted_tokens: usize,
    pub total_tokens: usize,
    pub used_parallel: bool,
    pub fallback_reason: Option<String>,
    pub reduced_logits: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerShard {
    pub shard_index: usize,
    pub total_shards: usize,
    pub request: InferenceRequest,
}

#[async_trait]
pub trait CoVerifierClient: Send + Sync {
    async fn list_capability_peers(&self, capability: &str) -> Result<Vec<String>>;
    async fn send_layer_shard(&self, peer_id: &str, shard: LayerShard) -> Result<Vec<Vec<f32>>>;
    async fn local_verify(&self, request: &InferenceRequest) -> Result<Vec<Vec<f32>>>;
}

pub struct TensorParallelCoordinator<C: CoVerifierClient> {
    local_peer_id: String,
    parallel_degree: usize,
    co_verifier_timeout_ms: u64,
    client: C,
}

impl<C: CoVerifierClient> TensorParallelCoordinator<C> {
    pub fn new(local_peer_id: String, config: TensorParallelConfig, client: C) -> Self {
        Self {
            local_peer_id,
            parallel_degree: config.degree.max(1),
            co_verifier_timeout_ms: config.co_verifier_timeout_ms.max(1),
            client,
        }
    }

    pub async fn discover_co_verifiers(&self) -> Vec<String> {
        let wanted = self.parallel_degree.saturating_sub(1);
        let Ok(mut peers) = self.client.list_capability_peers("co-verifier").await else {
            return Vec::new();
        };
        peers.retain(|peer| peer != &self.local_peer_id);
        peers.truncate(wanted);
        peers
    }

    pub async fn verify_parallel(
        &self,
        request: InferenceRequest,
        co_verifiers: Vec<String>,
    ) -> VerificationResult {
        if co_verifiers.is_empty() {
            return self.fallback_single(request, Some("no_co_verifiers".to_string())).await;
        }

        let shard_count = co_verifiers.len() + 1;
        let mut partials: Vec<Vec<Vec<f32>>> = Vec::with_capacity(shard_count);

        match self.client.local_verify(&request).await {
            Ok(local_logits) => partials.push(local_logits),
            Err(error) => {
                return VerificationResult {
                    accepted_tokens: 0,
                    total_tokens: request.draft_tokens.len(),
                    used_parallel: false,
                    fallback_reason: Some(format!("local_verify_failed: {error}")),
                    reduced_logits: Vec::new(),
                };
            }
        }

        for (index, peer) in co_verifiers.iter().enumerate() {
            let shard = LayerShard {
                shard_index: index + 1,
                total_shards: shard_count,
                request: request.clone(),
            };
            let fut = self.client.send_layer_shard(peer, shard);
            let timed = timeout(Duration::from_millis(self.co_verifier_timeout_ms), fut).await;
            match timed {
                Ok(Ok(logits)) => partials.push(logits),
                Ok(Err(error)) => {
                    return self
                        .fallback_single(request, Some(format!("co_verifier_error:{peer}:{error}")))
                        .await;
                }
                Err(_) => {
                    return self
                        .fallback_single(request, Some(format!("co_verifier_timeout:{peer}")))
                        .await;
                }
            }
        }

        let reduced = all_reduce_logits(&partials);
        let accepted = acceptance_check(&request.draft_tokens, &reduced);

        VerificationResult {
            accepted_tokens: accepted,
            total_tokens: request.draft_tokens.len(),
            used_parallel: true,
            fallback_reason: None,
            reduced_logits: reduced,
        }
    }

    async fn fallback_single(
        &self,
        request: InferenceRequest,
        reason: Option<String>,
    ) -> VerificationResult {
        let reduced = self.client.local_verify(&request).await.unwrap_or_default();
        let accepted = acceptance_check(&request.draft_tokens, &reduced);
        VerificationResult {
            accepted_tokens: accepted,
            total_tokens: request.draft_tokens.len(),
            used_parallel: false,
            fallback_reason: reason,
            reduced_logits: reduced,
        }
    }
}

fn all_reduce_logits(partials: &[Vec<Vec<f32>>]) -> Vec<Vec<f32>> {
    if partials.is_empty() {
        return Vec::new();
    }
    let token_len = partials[0].len();
    let vocab_len = partials[0].first().map(|row| row.len()).unwrap_or(0);
    let mut summed = vec![vec![0.0f32; vocab_len]; token_len];

    for partial in partials {
        for (t, row) in partial.iter().enumerate().take(token_len) {
            for (v, value) in row.iter().enumerate().take(vocab_len) {
                summed[t][v] += *value;
            }
        }
    }

    let denom = partials.len() as f32;
    for row in &mut summed {
        for value in row.iter_mut() {
            *value /= denom;
        }
        normalize_row(row);
    }

    summed
}

fn normalize_row(row: &mut [f32]) {
    if row.is_empty() {
        return;
    }
    let max = row
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, |a, b| if a > b { a } else { b });
    let mut sum = 0.0f32;
    for value in row.iter_mut() {
        *value = (*value - max).exp();
        sum += *value;
    }
    if sum <= 0.0 {
        return;
    }
    for value in row.iter_mut() {
        *value /= sum;
    }
}

fn acceptance_check(draft_tokens: &[i32], reduced_logits: &[Vec<f32>]) -> usize {
    let mut accepted = 0usize;
    for (idx, draft_token) in draft_tokens.iter().enumerate() {
        let Some(row) = reduced_logits.get(idx) else {
            break;
        };
        let mut best_idx = 0usize;
        let mut best_val = f32::NEG_INFINITY;
        for (i, value) in row.iter().enumerate() {
            if *value > best_val {
                best_val = *value;
                best_idx = i;
            }
        }
        if *draft_token == best_idx as i32 {
            accepted += 1;
        } else {
            break;
        }
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Clone)]
    struct MockClient {
        peers: Vec<String>,
        local: Vec<Vec<f32>>,
        remote: Vec<Vec<f32>>,
        fail_remote: bool,
        sent_protocols: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl CoVerifierClient for MockClient {
        async fn list_capability_peers(&self, _capability: &str) -> Result<Vec<String>> {
            Ok(self.peers.clone())
        }

        async fn send_layer_shard(&self, _peer_id: &str, _shard: LayerShard) -> Result<Vec<Vec<f32>>> {
            let mut guard = self.sent_protocols.lock().await;
            guard.push(TENSOR_PARALLEL_PROTOCOL.to_string());
            drop(guard);
            if self.fail_remote {
                anyhow::bail!("remote failed")
            }
            Ok(self.remote.clone())
        }

        async fn local_verify(&self, _request: &InferenceRequest) -> Result<Vec<Vec<f32>>> {
            Ok(self.local.clone())
        }
    }

    #[tokio::test]
    async fn discovers_up_to_degree_minus_one() {
        let client = MockClient {
            peers: vec!["peer-a".to_string(), "peer-b".to_string(), "peer-c".to_string()],
            local: vec![vec![0.9, 0.1]],
            remote: vec![vec![0.8, 0.2]],
            fail_remote: false,
            sent_protocols: Arc::new(Mutex::new(Vec::new())),
        };
        let coordinator = TensorParallelCoordinator::new(
            "peer-local".to_string(),
            TensorParallelConfig {
                enabled: true,
                degree: 3,
                co_verifier_timeout_ms: 500,
            },
            client,
        );
        let peers = coordinator.discover_co_verifiers().await;
        assert_eq!(peers.len(), 2);
    }

    #[tokio::test]
    async fn falls_back_on_remote_error() {
        let client = MockClient {
            peers: vec!["peer-a".to_string()],
            local: vec![vec![0.9, 0.1], vec![0.9, 0.1]],
            remote: vec![vec![0.9, 0.1], vec![0.9, 0.1]],
            fail_remote: true,
            sent_protocols: Arc::new(Mutex::new(Vec::new())),
        };
        let coordinator = TensorParallelCoordinator::new(
            "peer-local".to_string(),
            TensorParallelConfig {
                enabled: true,
                degree: 2,
                co_verifier_timeout_ms: 500,
            },
            client,
        );
        let request = InferenceRequest {
            request_id: "r1".to_string(),
            prompt_context: "x".to_string(),
            draft_tokens: vec![0, 0],
        };
        let result = coordinator
            .verify_parallel(request, vec!["peer-a".to_string()])
            .await;
        assert!(!result.used_parallel);
        assert!(result.fallback_reason.unwrap_or_default().contains("co_verifier_error"));
    }
}
