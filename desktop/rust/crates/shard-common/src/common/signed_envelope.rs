//! Cryptographic signed envelope for zero-trust message authentication.
//!
//! Every control/data-plane message across the Shard mesh must be wrapped in a
//! `SignedEnvelope`. The envelope provides:
//! - **Authentication**: Ed25519 signature over canonical payload hash
//! - **Integrity**: SHA-256 payload hash verified before signature check
//! - **Replay protection**: Sliding-window nonce tracker per sender pubkey
//! - **Freshness**: Configurable timestamp window rejects stale messages

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Rejection Reasons ──────────────────────────────────────────────────────

/// Typed rejection reasons for envelope verification failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvelopeRejection {
    /// Signer public key could not be decoded or is malformed.
    InvalidSignerKey,
    /// Ed25519 signature verification failed.
    InvalidSignature,
    /// Payload hash recomputation does not match envelope hash.
    PayloadHashMismatch,
    /// Nonce has already been seen within the sliding window (replay attempt).
    ReplayedNonce,
    /// Timestamp is outside the acceptable freshness window.
    ExpiredTimestamp,
    /// Signer public key is not in the set of known/registered keys.
    UnknownKey,
    /// PoW challenge has not been completed for this sender.
    PowNotVerified,
}

impl std::fmt::Display for EnvelopeRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSignerKey => write!(f, "invalid signer public key"),
            Self::InvalidSignature => write!(f, "signature verification failed"),
            Self::PayloadHashMismatch => write!(f, "payload hash mismatch"),
            Self::ReplayedNonce => write!(f, "replayed nonce"),
            Self::ExpiredTimestamp => write!(f, "expired timestamp"),
            Self::UnknownKey => write!(f, "unknown signer key"),
            Self::PowNotVerified => write!(f, "PoW challenge not completed"),
        }
    }
}

// ─── Signed Envelope ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedEnvelope<T> {
    pub signer_pubkey_hex: String,
    pub nonce: u64,
    pub timestamp_ms: u128,
    pub payload: T,
    pub payload_hash_hex: String,
    pub signature_hex: String,
}

impl<T: Serialize + Clone> SignedEnvelope<T> {
    /// Create a signed envelope wrapping a payload.
    pub fn sign(payload: T, signing_key: &SigningKey, nonce: u64, timestamp_ms: u128) -> Self {
        let signer_pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let payload_hash_hex = payload_hash(&payload);
        let signing_body = signing_body(&signer_pubkey_hex, nonce, timestamp_ms, &payload_hash_hex);
        let signature = signing_key.sign(signing_body.as_bytes());

        Self {
            signer_pubkey_hex,
            nonce,
            timestamp_ms,
            payload,
            payload_hash_hex,
            signature_hex: hex::encode(signature.to_bytes()),
        }
    }
}

impl<T: Serialize + DeserializeOwned + Clone> SignedEnvelope<T> {
    /// Verify the envelope's signature and payload integrity.
    /// Does NOT check nonce replay or timestamp freshness — use
    /// `EnvelopeVerifier::verify_full()` for complete zero-trust validation.
    pub fn verify_signature(&self) -> Result<(), EnvelopeRejection> {
        // Decode and validate public key
        let pubkey_bytes = hex::decode(&self.signer_pubkey_hex)
            .map_err(|_| EnvelopeRejection::InvalidSignerKey)?;
        if pubkey_bytes.len() != 32 {
            return Err(EnvelopeRejection::InvalidSignerKey);
        }
        let mut pubkey_arr = [0u8; 32];
        pubkey_arr.copy_from_slice(&pubkey_bytes);
        let verifying_key = VerifyingKey::from_bytes(&pubkey_arr)
            .map_err(|_| EnvelopeRejection::InvalidSignerKey)?;

        // Verify payload hash
        let recomputed_hash = payload_hash(&self.payload);
        if recomputed_hash != self.payload_hash_hex {
            return Err(EnvelopeRejection::PayloadHashMismatch);
        }

        // Verify signature
        let sig_bytes =
            hex::decode(&self.signature_hex).map_err(|_| EnvelopeRejection::InvalidSignature)?;
        if sig_bytes.len() != 64 {
            return Err(EnvelopeRejection::InvalidSignature);
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_arr);

        let body = signing_body(
            &self.signer_pubkey_hex,
            self.nonce,
            self.timestamp_ms,
            &self.payload_hash_hex,
        );
        verifying_key
            .verify(body.as_bytes(), &signature)
            .map_err(|_| EnvelopeRejection::InvalidSignature)
    }

    /// Legacy compat: returns Result<(), String>
    pub fn verify(&self) -> Result<(), String> {
        self.verify_signature().map_err(|e| e.to_string())
    }
}

// ─── Envelope Verifier (stateful, tracks nonces + timestamps) ───────────────

/// Configuration for envelope verification.
#[derive(Debug, Clone)]
pub struct VerifierConfig {
    /// Maximum age of a message timestamp before rejection (milliseconds).
    pub max_timestamp_age_ms: u128,
    /// Maximum number of nonces to track per sender (sliding window size).
    pub nonce_window_size: usize,
}

impl Default for VerifierConfig {
    fn default() -> Self {
        Self {
            max_timestamp_age_ms: 30_000, // 30 seconds
            nonce_window_size: 1024,
        }
    }
}

/// Stateful envelope verifier with replay protection and timestamp freshness.
pub struct EnvelopeVerifier {
    config: VerifierConfig,
    /// Per-sender nonce sliding window: maps pubkey hex → recent nonces
    nonce_windows: HashMap<String, VecDeque<u64>>,
}

impl EnvelopeVerifier {
    pub fn new(config: VerifierConfig) -> Self {
        Self {
            config,
            nonce_windows: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(VerifierConfig::default())
    }

    /// Full zero-trust verification: signature + payload hash + timestamp + nonce replay.
    pub fn verify_full<T: Serialize + DeserializeOwned + Clone>(
        &mut self,
        envelope: &SignedEnvelope<T>,
    ) -> Result<(), EnvelopeRejection> {
        // 1. Signature + payload integrity
        envelope.verify_signature()?;

        // 2. Timestamp freshness
        let now = now_ms();
        let age = now.saturating_sub(envelope.timestamp_ms);
        if age > self.config.max_timestamp_age_ms {
            return Err(EnvelopeRejection::ExpiredTimestamp);
        }
        // Also reject messages from the future (> 5s clock skew tolerance)
        if envelope.timestamp_ms > now + 5_000 {
            return Err(EnvelopeRejection::ExpiredTimestamp);
        }

        // 3. Nonce replay protection
        let window = self
            .nonce_windows
            .entry(envelope.signer_pubkey_hex.clone())
            .or_default();

        if window.contains(&envelope.nonce) {
            return Err(EnvelopeRejection::ReplayedNonce);
        }

        // Add nonce and evict oldest if window is full
        window.push_back(envelope.nonce);
        if window.len() > self.config.nonce_window_size {
            window.pop_front();
        }

        Ok(())
    }

    /// Clear nonce history for a specific sender (e.g., on key rotation).
    pub fn clear_sender(&mut self, pubkey_hex: &str) {
        self.nonce_windows.remove(pubkey_hex);
    }

    /// Number of tracked senders.
    pub fn tracked_senders(&self) -> usize {
        self.nonce_windows.len()
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn payload_hash<T: Serialize>(payload: &T) -> String {
    let bytes = serde_json::to_vec(payload).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

fn signing_body(
    signer_pubkey_hex: &str,
    nonce: u64,
    timestamp_ms: u128,
    payload_hash_hex: &str,
) -> String {
    format!("{signer_pubkey_hex}|{nonce}|{timestamp_ms}|{payload_hash_hex}")
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
    use ed25519_dalek::SigningKey;
    use rand::RngCore;

    fn key() -> SigningKey {
        let mut sk = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut sk);
        SigningKey::from_bytes(&sk)
    }

    #[test]
    fn signed_envelope_round_trip() {
        let signing = key();
        let payload = serde_json::json!({"job_id": "abc", "tokens": 8});
        let envelope = SignedEnvelope::sign(payload, &signing, 1, now_ms());
        assert!(envelope.verify_signature().is_ok());
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let signing = key();
        let payload = serde_json::json!({"job_id": "abc", "tokens": 8});
        let mut envelope = SignedEnvelope::sign(payload, &signing, 1, now_ms());
        envelope.payload = serde_json::json!({"job_id": "abc", "tokens": 9});
        assert_eq!(
            envelope.verify_signature().unwrap_err(),
            EnvelopeRejection::PayloadHashMismatch
        );
    }

    #[test]
    fn wrong_key_fails_verification() {
        let signing = key();
        let other_key = key();
        let payload = serde_json::json!({"data": "test"});
        let mut envelope = SignedEnvelope::sign(payload, &signing, 1, now_ms());
        // Replace pubkey with a different key
        envelope.signer_pubkey_hex = hex::encode(other_key.verifying_key().to_bytes());
        assert_eq!(
            envelope.verify_signature().unwrap_err(),
            EnvelopeRejection::InvalidSignature
        );
    }

    #[test]
    fn replay_nonce_rejected() {
        let signing = key();
        let now = now_ms();
        let payload = serde_json::json!({"data": "test"});
        let envelope = SignedEnvelope::sign(payload, &signing, 42, now);

        let mut verifier = EnvelopeVerifier::with_defaults();
        assert!(verifier.verify_full(&envelope).is_ok());
        // Same nonce again should be rejected
        assert_eq!(
            verifier.verify_full(&envelope).unwrap_err(),
            EnvelopeRejection::ReplayedNonce
        );
    }

    #[test]
    fn expired_timestamp_rejected() {
        let signing = key();
        let old_timestamp = now_ms().saturating_sub(60_000); // 60s ago
        let payload = serde_json::json!({"data": "old"});
        let envelope = SignedEnvelope::sign(payload, &signing, 1, old_timestamp);

        let mut verifier = EnvelopeVerifier::with_defaults();
        assert_eq!(
            verifier.verify_full(&envelope).unwrap_err(),
            EnvelopeRejection::ExpiredTimestamp
        );
    }

    #[test]
    fn future_timestamp_rejected() {
        let signing = key();
        let future_timestamp = now_ms() + 30_000; // 30s in the future
        let payload = serde_json::json!({"data": "future"});
        let envelope = SignedEnvelope::sign(payload, &signing, 1, future_timestamp);

        let mut verifier = EnvelopeVerifier::with_defaults();
        assert_eq!(
            verifier.verify_full(&envelope).unwrap_err(),
            EnvelopeRejection::ExpiredTimestamp
        );
    }

    #[test]
    fn nonce_window_eviction() {
        let signing = key();
        let config = VerifierConfig {
            max_timestamp_age_ms: 60_000,
            nonce_window_size: 3,
        };
        let mut verifier = EnvelopeVerifier::new(config);
        let now = now_ms();

        // Fill the window with nonces 1, 2, 3
        for nonce in 1..=3 {
            let envelope =
                SignedEnvelope::sign(serde_json::json!({"n": nonce}), &signing, nonce, now);
            assert!(verifier.verify_full(&envelope).is_ok());
        }

        // Nonce 1 should still be in the window
        let replay = SignedEnvelope::sign(serde_json::json!({"n": 1}), &signing, 1, now);
        assert_eq!(
            verifier.verify_full(&replay).unwrap_err(),
            EnvelopeRejection::ReplayedNonce
        );

        // Push nonce 4 — this should evict nonce 1
        let env4 = SignedEnvelope::sign(serde_json::json!({"n": 4}), &signing, 4, now);
        assert!(verifier.verify_full(&env4).is_ok());

        // Now nonce 1 should be accepted again (evicted from window)
        let replay1 = SignedEnvelope::sign(serde_json::json!({"n": 1}), &signing, 1, now);
        assert!(verifier.verify_full(&replay1).is_ok());
    }

    #[test]
    fn legacy_verify_compat() {
        let signing = key();
        let payload = serde_json::json!({"job_id": "abc", "tokens": 8});
        let envelope = SignedEnvelope::sign(payload, &signing, 1, 1234);
        assert!(envelope.verify().is_ok());
    }

    #[test]
    fn tampered_signature_fails_verification() {
        let signing = key();
        let payload = serde_json::json!({"job_id": "abc", "tokens": 8});
        let mut envelope = SignedEnvelope::sign(payload, &signing, 1, now_ms());
        let mut sig = hex::decode(envelope.signature_hex.clone()).expect("signature should decode");
        sig[0] ^= 0xFF;
        envelope.signature_hex = hex::encode(sig);
        assert_eq!(
            envelope.verify_signature().unwrap_err(),
            EnvelopeRejection::InvalidSignature
        );
    }

    #[test]
    fn malformed_signature_hex_fails_verification() {
        let signing = key();
        let payload = serde_json::json!({"job_id": "abc", "tokens": 8});
        let mut envelope = SignedEnvelope::sign(payload, &signing, 1, now_ms());
        envelope.signature_hex = "not-hex".to_string();
        assert_eq!(
            envelope.verify_signature().unwrap_err(),
            EnvelopeRejection::InvalidSignature
        );
    }

    #[test]
    fn replay_protection_is_scoped_per_signer() {
        let signing_a = key();
        let signing_b = key();
        let now = now_ms();
        let payload = serde_json::json!({"data": "scoped-nonce"});
        let envelope_a = SignedEnvelope::sign(payload.clone(), &signing_a, 7, now);
        let envelope_b = SignedEnvelope::sign(payload, &signing_b, 7, now);

        let mut verifier = EnvelopeVerifier::with_defaults();
        assert!(verifier.verify_full(&envelope_a).is_ok());
        assert_eq!(
            verifier.verify_full(&envelope_a).unwrap_err(),
            EnvelopeRejection::ReplayedNonce
        );
        assert!(
            verifier.verify_full(&envelope_b).is_ok(),
            "same nonce must be accepted for a different signer"
        );
    }
}
