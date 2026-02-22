use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    #[default]
    Gateway,
    Shard,
    Scout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRuntimeConfig {
    pub node_role: NodeRole,
    pub max_cpu: f32,
    pub max_gpu: f32,
    pub idle_only: bool,
    pub load_threshold_cutoff: f32,
    pub heartbeat_interval_seconds: u64,
    pub ice_servers: Vec<String>,
}

impl Default for NodeRuntimeConfig {
    fn default() -> Self {
        Self {
            node_role: NodeRole::Gateway,
            max_cpu: 0.5,
            max_gpu: 0.5,
            idle_only: false,
            load_threshold_cutoff: 0.85,
            heartbeat_interval_seconds: 10,
            ice_servers: Vec::new(),
        }
    }
}

impl NodeRuntimeConfig {
    pub fn load_with_env(path: &Path) -> Self {
        let mut config = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_yaml::from_str::<NodeRuntimeConfig>(&raw).ok())
            .unwrap_or_default();

        if let Ok(value) = std::env::var("SHARD_NODE_ROLE") {
            config.node_role = match value.to_lowercase().as_str() {
                "scout" => NodeRole::Scout,
                "shard" => NodeRole::Shard,
                _ => NodeRole::Gateway,
            };
        }
        if let Ok(value) = std::env::var("SHARD_MAX_CPU") {
            if let Ok(parsed) = value.parse::<f32>() {
                config.max_cpu = parsed.clamp(0.1, 1.0);
            }
        }
        if let Ok(value) = std::env::var("SHARD_MAX_GPU") {
            if let Ok(parsed) = value.parse::<f32>() {
                config.max_gpu = parsed.clamp(0.1, 1.0);
            }
        }
        if let Ok(value) = std::env::var("SHARD_IDLE_ONLY") {
            config.idle_only = matches!(value.as_str(), "1" | "true" | "TRUE");
        }
        if let Ok(value) = std::env::var("SHARD_LOAD_THRESHOLD_CUTOFF") {
            if let Ok(parsed) = value.parse::<f32>() {
                config.load_threshold_cutoff = parsed.clamp(0.1, 1.0);
            }
        }
        if let Ok(value) = std::env::var("SHARD_HEARTBEAT_INTERVAL_SECONDS") {
            if let Ok(parsed) = value.parse::<u64>() {
                config.heartbeat_interval_seconds = parsed.clamp(2, 300);
            }
        }
        if let Ok(value) = std::env::var("SHARD_ICE_SERVERS") {
            config.ice_servers = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        config
    }
}
