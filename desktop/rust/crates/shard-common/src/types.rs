use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRequest {
    pub request_id: String,
    pub prompt_context: String,
    pub min_tokens: i32,
    #[serde(default)]
    pub created_at_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResponse {
    pub request_id: String,
    pub peer_id: String,
    pub draft_tokens: Vec<String>,
    pub latency_ms: f32,
    #[serde(default)]
    pub created_at_ms: Option<u128>,
}
