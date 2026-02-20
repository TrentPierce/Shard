//! Hashcash-style Proof-of-Work challenge for Sybil resistance.
//!
//! When a new Scout connects, the Verifier issues a `PowChallenge`. The Scout
//! must find a nonce such that `SHA-256(challenge_bytes || nonce_bytes)` has at
//! least `difficulty` leading zero bits. Alternatively, Scouts may submit a
//! Cloudflare Turnstile or reCAPTCHA v3 token to bypass PoW entirely.
//!
//! Difficulty is hardware-aware:
//! - concurrency >= 8  → 20 bits (~1s on desktop)
//! - concurrency 4–7   → 16 bits (~0.5s on laptop)
//! - concurrency <= 3  → 12 bits (~0.2s on mobile)

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Challenge issued by the Verifier to a new Scout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowChallenge {
    pub challenge_bytes_hex: String,
    pub difficulty: u8,
    pub issued_at_ms: u128,
    pub expires_at_ms: u128,
    pub accepts_captcha_bypass: bool,
}

/// Solution submitted by a Scout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowSolution {
    pub nonce: u64,
    pub hash_hex: String,
}

/// Captcha token submitted as an alternative to PoW.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaToken {
    pub provider: CaptchaProvider,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptchaProvider {
    Turnstile,
    RecaptchaV3,
}

/// Outcome of verifying a PoW solution or captcha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowVerifyResult {
    Accepted,
    InsufficientDifficulty,
    HashMismatch,
    ChallengeExpired,
    ChallengeNotFound,
}

// ─── Difficulty Scaling ─────────────────────────────────────────────────────

/// Determine PoW difficulty based on hardware concurrency reported by the Scout.
pub fn difficulty_for_concurrency(concurrency: u32) -> u8 {
    match concurrency {
        0..=3 => 12,
        4..=7 => 16,
        _ => 20,
    }
}

/// Increase difficulty under Sybil alert conditions.
/// Each alert level adds 4 bits of difficulty, capped at 28.
pub fn escalated_difficulty(base: u8, alert_level: u8) -> u8 {
    (base + alert_level.saturating_mul(4)).min(28)
}

// ─── Challenge Manager ──────────────────────────────────────────────────────

/// Tracks issued PoW challenges and verified peers.
pub struct PowChallengeManager {
    /// Pending challenges: maps peer pubkey hex → challenge
    pending: HashMap<String, PowChallenge>,
    /// Verified peers: maps peer pubkey hex → expiry timestamp ms
    verified: HashMap<String, u128>,
    /// TTL for verified status (default 1 hour)
    verified_ttl_ms: u128,
    /// Current Sybil alert level (0 = normal)
    sybil_alert_level: u8,
}

impl PowChallengeManager {
    pub fn new(verified_ttl_ms: u128) -> Self {
        Self {
            pending: HashMap::new(),
            verified: HashMap::new(),
            verified_ttl_ms,
            sybil_alert_level: 0,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(3_600_000) // 1 hour
    }

    /// Issue a new challenge for a peer.
    pub fn issue_challenge(
        &mut self,
        peer_pubkey_hex: &str,
        hardware_concurrency: u32,
        challenge_lifetime_ms: u128,
        accepts_captcha: bool,
    ) -> PowChallenge {
        let now = now_ms();
        let base_difficulty = difficulty_for_concurrency(hardware_concurrency);
        let difficulty = escalated_difficulty(base_difficulty, self.sybil_alert_level);

        // Generate random challenge bytes
        let challenge_bytes: [u8; 32] = rand_bytes();

        let challenge = PowChallenge {
            challenge_bytes_hex: hex::encode(challenge_bytes),
            difficulty,
            issued_at_ms: now,
            expires_at_ms: now + challenge_lifetime_ms,
            accepts_captcha_bypass: accepts_captcha,
        };

        self.pending
            .insert(peer_pubkey_hex.to_string(), challenge.clone());
        challenge
    }

    /// Verify a PoW solution from a peer.
    pub fn verify_solution(
        &mut self,
        peer_pubkey_hex: &str,
        solution: &PowSolution,
    ) -> PowVerifyResult {
        let now = now_ms();

        let Some(challenge) = self.pending.get(peer_pubkey_hex) else {
            return PowVerifyResult::ChallengeNotFound;
        };

        if now > challenge.expires_at_ms {
            self.pending.remove(peer_pubkey_hex);
            return PowVerifyResult::ChallengeExpired;
        }

        let challenge_bytes = match hex::decode(&challenge.challenge_bytes_hex) {
            Ok(b) => b,
            Err(_) => return PowVerifyResult::ChallengeNotFound,
        };

        // Recompute hash: SHA-256(challenge_bytes || nonce_le_bytes)
        let expected_hash = compute_pow_hash(&challenge_bytes, solution.nonce);
        let expected_hex = hex::encode(expected_hash);

        if expected_hex != solution.hash_hex {
            return PowVerifyResult::HashMismatch;
        }

        // Check leading zero bits
        let leading_zeros = count_leading_zero_bits(&expected_hash);
        if leading_zeros < challenge.difficulty as u32 {
            return PowVerifyResult::InsufficientDifficulty;
        }

        // PoW accepted — mark peer as verified
        self.pending.remove(peer_pubkey_hex);
        self.verified
            .insert(peer_pubkey_hex.to_string(), now + self.verified_ttl_ms);
        PowVerifyResult::Accepted
    }

    /// Mark a peer as verified via captcha bypass (caller must validate token externally).
    pub fn accept_captcha_bypass(&mut self, peer_pubkey_hex: &str) {
        let now = now_ms();
        self.pending.remove(peer_pubkey_hex);
        self.verified
            .insert(peer_pubkey_hex.to_string(), now + self.verified_ttl_ms);
    }

    /// Check if a peer has been PoW-verified and still within TTL.
    pub fn is_verified(&self, peer_pubkey_hex: &str) -> bool {
        match self.verified.get(peer_pubkey_hex) {
            Some(&expires) => now_ms() < expires,
            None => false,
        }
    }

    /// Set the Sybil alert level (0 = normal, each level adds 4 bits difficulty).
    pub fn set_sybil_alert_level(&mut self, level: u8) {
        self.sybil_alert_level = level;
    }

    pub fn sybil_alert_level(&self) -> u8 {
        self.sybil_alert_level
    }

    /// Prune expired verified entries.
    pub fn prune_expired(&mut self) {
        let now = now_ms();
        self.verified.retain(|_, expires| *expires > now);
        self.pending.retain(|_, c| c.expires_at_ms > now);
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Compute SHA-256(challenge_bytes || nonce_le_bytes).
pub fn compute_pow_hash(challenge_bytes: &[u8], nonce: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(challenge_bytes);
    hasher.update(nonce.to_le_bytes());
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Count leading zero bits in a byte slice.
pub fn count_leading_zero_bits(hash: &[u8]) -> u32 {
    let mut count = 0u32;
    for byte in hash {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

/// Solve a PoW challenge (for testing — real solving happens in browser Web Worker).
pub fn solve_challenge(challenge_bytes: &[u8], difficulty: u8) -> Option<PowSolution> {
    for nonce in 0..u64::MAX {
        let hash = compute_pow_hash(challenge_bytes, nonce);
        if count_leading_zero_bits(&hash) >= difficulty as u32 {
            return Some(PowSolution {
                nonce,
                hash_hex: hex::encode(hash),
            });
        }
    }
    None
}

fn rand_bytes() -> [u8; 32] {
    let mut buf = [0u8; 32];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_scaling_by_concurrency() {
        assert_eq!(difficulty_for_concurrency(1), 12);
        assert_eq!(difficulty_for_concurrency(2), 12);
        assert_eq!(difficulty_for_concurrency(3), 12);
        assert_eq!(difficulty_for_concurrency(4), 16);
        assert_eq!(difficulty_for_concurrency(6), 16);
        assert_eq!(difficulty_for_concurrency(7), 16);
        assert_eq!(difficulty_for_concurrency(8), 20);
        assert_eq!(difficulty_for_concurrency(16), 20);
    }

    #[test]
    fn escalated_difficulty_scales() {
        assert_eq!(escalated_difficulty(12, 0), 12);
        assert_eq!(escalated_difficulty(12, 1), 16);
        assert_eq!(escalated_difficulty(12, 2), 20);
        assert_eq!(escalated_difficulty(20, 3), 28); // capped at 28
        assert_eq!(escalated_difficulty(20, 10), 28); // capped at 28
    }

    #[test]
    fn solve_and_verify_low_difficulty() {
        let mut manager = PowChallengeManager::with_defaults();
        let challenge = manager.issue_challenge("peer_a", 8, 60_000, false);

        // Override difficulty to something solvable quickly in tests
        let challenge_bytes = hex::decode(&challenge.challenge_bytes_hex).unwrap();
        let solution = solve_challenge(&challenge_bytes, 4).expect("should solve difficulty 4");

        // Manually create a challenge with difficulty 4
        let test_challenge = PowChallenge {
            difficulty: 4,
            ..challenge
        };
        manager.pending.insert("peer_a".to_string(), test_challenge);

        let result = manager.verify_solution("peer_a", &solution);
        assert_eq!(result, PowVerifyResult::Accepted);
        assert!(manager.is_verified("peer_a"));
    }

    #[test]
    fn insufficient_difficulty_rejected() {
        let mut manager = PowChallengeManager::with_defaults();
        let challenge = manager.issue_challenge("peer_b", 8, 60_000, false);

        // Submit a solution with nonce 0 — almost certainly won't meet difficulty 20
        let challenge_bytes = hex::decode(&challenge.challenge_bytes_hex).unwrap();
        let hash = compute_pow_hash(&challenge_bytes, 0);
        let zeros = count_leading_zero_bits(&hash);

        if zeros < 20 {
            let bad_solution = PowSolution {
                nonce: 0,
                hash_hex: hex::encode(hash),
            };
            let result = manager.verify_solution("peer_b", &bad_solution);
            assert_eq!(result, PowVerifyResult::InsufficientDifficulty);
        }
    }

    #[test]
    fn expired_challenge_rejected() {
        let mut manager = PowChallengeManager::with_defaults();
        let challenge = manager.issue_challenge("peer_c", 4, 60_000, false);
        let challenge_bytes = hex::decode(&challenge.challenge_bytes_hex).unwrap();
        let solution = solve_challenge(&challenge_bytes, 4).unwrap();

        // Manually backdate the challenge so it's guaranteed expired
        if let Some(c) = manager.pending.get_mut("peer_c") {
            c.expires_at_ms = 1; // epoch + 1ms = definitely expired
        }

        let result = manager.verify_solution("peer_c", &solution);
        assert_eq!(result, PowVerifyResult::ChallengeExpired);
    }

    #[test]
    fn captcha_bypass() {
        let mut manager = PowChallengeManager::with_defaults();
        let _challenge = manager.issue_challenge("peer_d", 2, 60_000, true);

        assert!(!manager.is_verified("peer_d"));
        manager.accept_captcha_bypass("peer_d");
        assert!(manager.is_verified("peer_d"));
    }

    #[test]
    fn unknown_peer_returns_not_found() {
        let mut manager = PowChallengeManager::with_defaults();
        let result = manager.verify_solution(
            "unknown",
            &PowSolution {
                nonce: 0,
                hash_hex: "0".repeat(64),
            },
        );
        assert_eq!(result, PowVerifyResult::ChallengeNotFound);
    }

    #[test]
    fn sybil_alert_increases_difficulty() {
        let mut manager = PowChallengeManager::with_defaults();
        manager.set_sybil_alert_level(2);

        let challenge = manager.issue_challenge("peer_e", 8, 60_000, false);
        // Base 20 + 2*4 = 28
        assert_eq!(challenge.difficulty, 28);
    }

    #[test]
    fn leading_zero_bits_count() {
        assert_eq!(count_leading_zero_bits(&[0x00, 0x00, 0xFF]), 16);
        assert_eq!(count_leading_zero_bits(&[0x00, 0x0F, 0xFF]), 12);
        assert_eq!(count_leading_zero_bits(&[0x01, 0xFF, 0xFF]), 7);
        assert_eq!(count_leading_zero_bits(&[0xFF, 0xFF, 0xFF]), 0);
        assert_eq!(count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x01]), 31);
    }
}
