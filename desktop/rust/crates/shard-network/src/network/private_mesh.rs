//! Private sub-mesh routing for enterprise prompt privacy.
//!
//! Enterprise customers can register corporate node groups so that prompts
//! marked as `sensitive` bypass the public Scout mesh entirely and route
//! only to authenticated private nodes or the centralized fallback API.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Routing mode for a request, determined by the `X-Shard-Route` header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RouteMode {
    /// Normal public Scout mesh dispatch.
    #[default]
    Public,
    /// Bypass public mesh; route to private nodes or centralized API only.
    Private,
}

/// A registered corporate peer group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorporatePeerGroup {
    /// API key that owns this peer group.
    pub api_key_hash: String,
    /// Set of node pubkey hexes that belong to this corporate group.
    pub node_pubkeys: HashSet<String>,
    /// Human-readable label for the group.
    pub label: Option<String>,
    pub created_at_ms: u128,
}

/// Registration request for a private corporate node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPrivateNodeRequest {
    pub api_key: String,
    pub node_pubkey_hex: String,
    pub label: Option<String>,
}

/// Routing decision for a private request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateRouteDecision {
    /// Route to specific corporate nodes.
    DispatchToPrivateNodes(Vec<String>),
    /// No private nodes available; fall back to centralized API.
    FallbackToCentralized,
}

// ─── Private Mesh Registry ──────────────────────────────────────────────────

/// Manages corporate peer group registrations and private routing decisions.
pub struct PrivateMeshRegistry {
    /// Maps API key hash → corporate peer group.
    groups: HashMap<String, CorporatePeerGroup>,
}

impl PrivateMeshRegistry {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Register a node as belonging to a corporate API key's private mesh.
    pub fn register_node(
        &mut self,
        api_key_hash: &str,
        node_pubkey_hex: &str,
        label: Option<String>,
    ) {
        let group = self
            .groups
            .entry(api_key_hash.to_string())
            .or_insert_with(|| CorporatePeerGroup {
                api_key_hash: api_key_hash.to_string(),
                node_pubkeys: HashSet::new(),
                label: label.clone(),
                created_at_ms: now_ms(),
            });
        group.node_pubkeys.insert(node_pubkey_hex.to_string());
    }

    /// Unregister a node from a corporate group.
    pub fn unregister_node(&mut self, api_key_hash: &str, node_pubkey_hex: &str) -> bool {
        if let Some(group) = self.groups.get_mut(api_key_hash) {
            group.node_pubkeys.remove(node_pubkey_hex);
            if group.node_pubkeys.is_empty() {
                self.groups.remove(api_key_hash);
            }
            true
        } else {
            false
        }
    }

    /// Decide how to route a private request for a given API key.
    /// Filters to only nodes that are currently connected.
    pub fn route_private(
        &self,
        api_key_hash: &str,
        connected_peer_pubkeys: &HashSet<String>,
    ) -> PrivateRouteDecision {
        if let Some(group) = self.groups.get(api_key_hash) {
            let available: Vec<String> = group
                .node_pubkeys
                .intersection(connected_peer_pubkeys)
                .cloned()
                .collect();

            if available.is_empty() {
                PrivateRouteDecision::FallbackToCentralized
            } else {
                PrivateRouteDecision::DispatchToPrivateNodes(available)
            }
        } else {
            // No registered corporate nodes — always centralized
            PrivateRouteDecision::FallbackToCentralized
        }
    }

    /// Get all registered groups (for admin dashboard).
    pub fn list_groups(&self) -> Vec<&CorporatePeerGroup> {
        self.groups.values().collect()
    }

    /// Number of registered groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
}

impl Default for PrivateMeshRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse route mode from HTTP header value.
pub fn parse_route_mode(header_value: Option<&str>) -> RouteMode {
    match header_value {
        Some(v) if v.eq_ignore_ascii_case("private") => RouteMode::Private,
        _ => RouteMode::Public,
    }
}

/// Hash an API key for storage (SHA-256, not reversible).
pub fn hash_api_key(api_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(api_key.as_bytes());
    hex::encode(hash)
}

fn now_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_route_private() {
        let mut registry = PrivateMeshRegistry::new();
        let api_hash = hash_api_key("sk_enterprise_123");

        registry.register_node(&api_hash, "node_a", Some("NYC Office".into()));
        registry.register_node(&api_hash, "node_b", Some("NYC Office".into()));

        // Both nodes connected
        let connected: HashSet<String> =
            ["node_a", "node_b"].iter().map(|s| s.to_string()).collect();
        match registry.route_private(&api_hash, &connected) {
            PrivateRouteDecision::DispatchToPrivateNodes(nodes) => {
                assert!(nodes.contains(&"node_a".to_string()));
                assert!(nodes.contains(&"node_b".to_string()));
            }
            _ => panic!("expected dispatch to private nodes"),
        }
    }

    #[test]
    fn no_connected_private_nodes_falls_back() {
        let mut registry = PrivateMeshRegistry::new();
        let api_hash = hash_api_key("sk_enterprise_456");

        registry.register_node(&api_hash, "node_x", None);

        // node_x is NOT in connected set
        let connected: HashSet<String> = ["other_node"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            registry.route_private(&api_hash, &connected),
            PrivateRouteDecision::FallbackToCentralized
        );
    }

    #[test]
    fn unregistered_api_key_falls_back() {
        let registry = PrivateMeshRegistry::new();
        let connected: HashSet<String> = ["node_a"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            registry.route_private("unknown_key_hash", &connected),
            PrivateRouteDecision::FallbackToCentralized
        );
    }

    #[test]
    fn unregister_node() {
        let mut registry = PrivateMeshRegistry::new();
        let api_hash = hash_api_key("sk_test");

        registry.register_node(&api_hash, "node_1", None);
        registry.register_node(&api_hash, "node_2", None);

        assert!(registry.unregister_node(&api_hash, "node_1"));
        assert_eq!(registry.group_count(), 1); // group still exists

        assert!(registry.unregister_node(&api_hash, "node_2"));
        assert_eq!(registry.group_count(), 0); // group removed (empty)
    }

    #[test]
    fn parse_route_header() {
        assert_eq!(parse_route_mode(Some("private")), RouteMode::Private);
        assert_eq!(parse_route_mode(Some("Private")), RouteMode::Private);
        assert_eq!(parse_route_mode(Some("PRIVATE")), RouteMode::Private);
        assert_eq!(parse_route_mode(Some("public")), RouteMode::Public);
        assert_eq!(parse_route_mode(None), RouteMode::Public);
        assert_eq!(parse_route_mode(Some("")), RouteMode::Public);
    }

    #[test]
    fn api_key_hashing() {
        let hash1 = hash_api_key("sk_test_123");
        let hash2 = hash_api_key("sk_test_123");
        let hash3 = hash_api_key("sk_test_456");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA-256 hex = 64 chars
    }
}
