use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkMode {
    Public,
    Private,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkPolicy {
    pub mode: NetworkMode,
    #[serde(default)]
    pub allowed_peer_cidrs: Vec<String>,
    #[serde(default)]
    pub blocked_peer_cidrs: Vec<String>,
    #[serde(default)]
    pub allowed_bootstrap_addrs: Vec<String>,
    #[serde(default)]
    pub reject_public_ips: bool,
    #[serde(default)]
    pub audit_log_blocked_connections: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny(String),
}

impl NetworkPolicy {
    pub fn from_yaml(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read network policy {}", path.display()))?;
        let policy: NetworkPolicy = serde_yaml::from_str(raw.as_str())
            .with_context(|| format!("failed to parse network policy {}", path.display()))?;
        Ok(policy)
    }

    pub fn check_connection(&self, peer_ip: IpAddr) -> PolicyDecision {
        if self.reject_public_ips && is_public_ip(peer_ip) {
            return PolicyDecision::Deny("public_ip_rejected".to_string());
        }

        for cidr in &self.blocked_peer_cidrs {
            if ip_in_cidr(peer_ip, cidr) {
                return PolicyDecision::Deny(format!("blocked_cidr:{cidr}"));
            }
        }

        if !self.allowed_peer_cidrs.is_empty() {
            let mut allowed = false;
            for cidr in &self.allowed_peer_cidrs {
                if ip_in_cidr(peer_ip, cidr) {
                    allowed = true;
                    break;
                }
            }
            if !allowed {
                return PolicyDecision::Deny("not_in_allowed_cidrs".to_string());
            }
        }

        PolicyDecision::Allow
    }
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified())
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unique_local())
        }
    }
}

fn ip_in_cidr(ip: IpAddr, cidr: &str) -> bool {
    let (net_part, prefix_part) = match cidr.split_once('/') {
        Some(parts) => parts,
        None => return false,
    };
    let Ok(prefix) = prefix_part.parse::<u8>() else {
        return false;
    };

    match (ip, net_part.parse::<IpAddr>()) {
        (IpAddr::V4(ipv4), Ok(IpAddr::V4(network))) => v4_in_cidr(ipv4, network, prefix),
        (IpAddr::V6(ipv6), Ok(IpAddr::V6(network))) => v6_in_cidr(ipv6, network, prefix),
        _ => false,
    }
}

fn v4_in_cidr(ip: Ipv4Addr, network: Ipv4Addr, prefix: u8) -> bool {
    if prefix > 32 {
        return false;
    }
    let ip_u = u32::from(ip);
    let net_u = u32::from(network);
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    (ip_u & mask) == (net_u & mask)
}

fn v6_in_cidr(ip: Ipv6Addr, network: Ipv6Addr, prefix: u8) -> bool {
    if prefix > 128 {
        return false;
    }
    let ip_u = u128::from(ip);
    let net_u = u128::from(network);
    let mask = if prefix == 0 { 0 } else { u128::MAX << (128 - prefix) };
    (ip_u & mask) == (net_u & mask)
}

#[cfg(test)]
mod tests {
    use super::{NetworkMode, NetworkPolicy, PolicyDecision};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn private_policy_blocks_public_ip() {
        let policy = NetworkPolicy {
            mode: NetworkMode::Private,
            allowed_peer_cidrs: vec!["10.0.0.0/8".to_string()],
            blocked_peer_cidrs: vec![],
            allowed_bootstrap_addrs: vec![],
            reject_public_ips: true,
            audit_log_blocked_connections: true,
        };
        let decision = policy.check_connection(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn private_policy_allows_allowed_subnet() {
        let policy = NetworkPolicy {
            mode: NetworkMode::Private,
            allowed_peer_cidrs: vec!["10.0.0.0/8".to_string()],
            blocked_peer_cidrs: vec![],
            allowed_bootstrap_addrs: vec![],
            reject_public_ips: false,
            audit_log_blocked_connections: true,
        };
        let decision = policy.check_connection(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3)));
        assert_eq!(decision, PolicyDecision::Allow);
    }
}
