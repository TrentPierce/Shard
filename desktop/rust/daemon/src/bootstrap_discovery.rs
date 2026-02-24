//! Bootstrap Discovery Module
//!
//! Provides functionality for discovering and advertising bootstrap peers.

use serde::{Deserialize, Serialize};

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
}

/// Request to register as a bootstrap peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapRegistration {
    pub peer_id: String,
    pub multiaddr: String,
    pub stability_score: u32,
    pub uptime_hours: u64,
    pub version: String,
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

    let peers: Vec<BootstrapPeer> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse bootstrap response: {}", e))?;

    Ok(peers)
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
