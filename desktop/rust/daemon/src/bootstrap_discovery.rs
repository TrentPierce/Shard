//! Bootstrap Discovery Module
//!
//! Provides functionality for discovering and advertising bootstrap peers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from bootstrap discovery endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapPeer {
    pub peer_id: String,
    pub multiaddr: String,
    #[serde(default)]
    pub stability_score: Option<u32>,
    #[serde(default)]
    pub uptime_hours: Option<u64>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub capability_tier: Option<String>,
    #[serde(default)]
    pub gpu_available: Option<bool>,
    #[serde(default)]
    pub accepts_scout_work: Option<bool>,
    #[serde(default)]
    pub public_api: Option<bool>,
}

/// Request to register as a bootstrap peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapRegistration {
    pub peer_id: String,
    pub multiaddr: String,
    pub stability_score: u32,
    pub uptime_hours: u64,
    pub version: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub capability_tier: Option<String>,
    #[serde(default)]
    pub gpu_available: Option<bool>,
    #[serde(default)]
    pub accepts_scout_work: Option<bool>,
    #[serde(default)]
    pub public_api: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootstrapEndpointEnvelope {
    #[serde(default)]
    known_bootstraps: Vec<BootstrapPeer>,
    #[serde(default)]
    registered_bootstraps: Vec<BootstrapPeer>,
}

fn dedupe_bootstrap_peers(peers: Vec<BootstrapPeer>) -> Vec<BootstrapPeer> {
    let mut by_peer: HashMap<String, BootstrapPeer> = HashMap::new();
    for peer in peers {
        let key = if peer.peer_id.trim().is_empty() {
            peer.multiaddr.clone()
        } else {
            peer.peer_id.clone()
        };
        by_peer.entry(key).or_insert(peer);
    }
    by_peer.into_values().collect()
}

fn parse_bootstrap_response(body: &str) -> Result<Vec<BootstrapPeer>, String> {
    if let Ok(peers) = serde_json::from_str::<Vec<BootstrapPeer>>(body) {
        return Ok(dedupe_bootstrap_peers(peers));
    }

    let envelope: BootstrapEndpointEnvelope = serde_json::from_str(body)
        .map_err(|e| format!("Failed to parse bootstrap response: {}", e))?;

    Ok(dedupe_bootstrap_peers(
        envelope
            .known_bootstraps
            .into_iter()
            .chain(envelope.registered_bootstraps)
            .collect(),
    ))
}

/// Fetch bootstrap peers from a discovery URL
pub async fn fetch_bootstrap_peers(url: &str) -> Result<Vec<BootstrapPeer>, String> {
    let client = reqwest::Client::new();

    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch bootstrap peers: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Bootstrap endpoint returned status: {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read bootstrap response: {}", e))?;

    parse_bootstrap_response(&body)
}

/// Register this node as a bootstrap peer
pub async fn register_as_bootstrap(
    url: &str,
    registration: &BootstrapRegistration,
) -> Result<(), String> {
    let client = reqwest::Client::new();

    client
        .post(url)
        .json(registration)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Failed to register as bootstrap: {}", e))?;

    Ok(())
}
