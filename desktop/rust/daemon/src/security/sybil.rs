use shard_common::common::pow_challenge::PowSolution;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub type WalletId = String;

#[derive(Debug, Clone)]
pub struct InvalidPoW {
    pub wallet: WalletId,
    pub reason: String,
}

#[derive(Debug)]
pub struct SybilDetector {
    max_nodes_per_subnet_per_hour: usize,
    min_pow_difficulty: u32,
    audit_log_path: PathBuf,
    subnet_registrations: HashMap<String, VecDeque<(Instant, WalletId)>>,
    flagged_wallets: HashSet<WalletId>,
    pow_verified_wallets: HashSet<WalletId>,
}

impl SybilDetector {
    pub fn new(
        max_nodes_per_subnet_per_hour: usize,
        min_pow_difficulty: u32,
        audit_log_path: PathBuf,
    ) -> Self {
        Self {
            max_nodes_per_subnet_per_hour,
            min_pow_difficulty,
            audit_log_path,
            subnet_registrations: HashMap::new(),
            flagged_wallets: HashSet::new(),
            pow_verified_wallets: HashSet::new(),
        }
    }

    pub fn record_new_node(&mut self, wallet: WalletId, ip: IpAddr, timestamp: Instant) {
        let subnet = subnet_from_ip(ip);
        let registrations = self
            .subnet_registrations
            .entry(subnet.clone())
            .or_insert_with(VecDeque::new);

        while let Some((seen_at, _)) = registrations.front() {
            if timestamp.duration_since(*seen_at) > Duration::from_secs(3600) {
                let _ = registrations.pop_front();
            } else {
                break;
            }
        }

        registrations.push_back((timestamp, wallet.clone()));
        let subnet_count = registrations.len();
        if subnet_count > self.max_nodes_per_subnet_per_hour {
            self.flagged_wallets.insert(wallet.clone());
            // Drop the mutable borrow before writing audit log.
            let _ = registrations;
            let _ = self.write_audit_log(
                "sybil_flag",
                &wallet,
                &subnet,
                subnet_count,
                "subnet_node_limit_exceeded",
            );
        }
    }

    pub fn is_flagged(&self, wallet: WalletId) -> bool {
        self.flagged_wallets.contains(&wallet)
    }

    pub fn is_pow_verified(&self, wallet: WalletId) -> bool {
        self.pow_verified_wallets.contains(&wallet)
    }

    pub fn record_pow_solution(
        &mut self,
        wallet: WalletId,
        solution: &PowSolution,
    ) -> Result<(), InvalidPoW> {
        let decoded = hex::decode(solution.hash_hex.as_str()).map_err(|_| InvalidPoW {
            wallet: wallet.clone(),
            reason: "hash_not_hex".to_string(),
        })?;
        if decoded.len() != 32 {
            return Err(InvalidPoW {
                wallet,
                reason: "hash_len_invalid".to_string(),
            });
        }
        let zero_bits = leading_zero_bits(decoded.as_slice());
        if zero_bits < self.min_pow_difficulty {
            return Err(InvalidPoW {
                wallet,
                reason: format!(
                    "difficulty_too_low expected>={} got={}",
                    self.min_pow_difficulty, zero_bits
                ),
            });
        }
        self.pow_verified_wallets.insert(wallet);
        Ok(())
    }

    fn write_audit_log(
        &self,
        event: &str,
        wallet: &str,
        ip_subnet: &str,
        subnet_node_count: usize,
        reason: &str,
    ) -> std::io::Result<()> {
        if let Some(parent) = self.audit_log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let line = serde_json::json!({
            "timestamp": ts_ms,
            "event": event,
            "wallet": wallet,
            "ip_subnet": ip_subnet,
            "subnet_node_count": subnet_node_count,
            "reason": reason,
        });

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.audit_log_path.as_path())?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}

fn subnet_from_ip(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            format!("{}.{}.{}", octets[0], octets[1], octets[2])
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}", segments[0], segments[1], segments[2], segments[3])
        }
    }
}

fn leading_zero_bits(data: &[u8]) -> u32 {
    let mut count = 0u32;
    for byte in data {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::SybilDetector;
    use shard_common::common::pow_challenge::PowSolution;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Instant;
    use tempfile::tempdir;

    #[test]
    fn sybil_detector_flags_subnet_overflow_and_writes_log() {
        let dir = tempdir().expect("tempdir");
        let audit = dir.path().join("shard_audit.jsonl");
        let mut detector = SybilDetector::new(2, 4, audit.clone());
        let now = Instant::now();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 1, 10));

        detector.record_new_node("w1".to_string(), ip, now);
        detector.record_new_node("w2".to_string(), ip, now);
        detector.record_new_node("w3".to_string(), ip, now);

        assert!(detector.is_flagged("w3".to_string()));
        let log = std::fs::read_to_string(audit).expect("audit log should exist");
        assert!(log.contains("sybil_flag"));
    }

    #[test]
    fn sybil_detector_pow_verification_enforces_difficulty() {
        let dir = tempdir().expect("tempdir");
        let mut detector = SybilDetector::new(5, 8, dir.path().join("audit.jsonl"));

        let weak = PowSolution {
            nonce: 1,
            hash_hex: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        };
        assert!(detector
            .record_pow_solution("wallet-a".to_string(), &weak)
            .is_err());

        let strong = PowSolution {
            nonce: 2,
            hash_hex: "0000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        };
        assert!(detector
            .record_pow_solution("wallet-a".to_string(), &strong)
            .is_ok());
        assert!(detector.is_pow_verified("wallet-a".to_string()));
    }
}
