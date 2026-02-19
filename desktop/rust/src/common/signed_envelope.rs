use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    pub fn verify(&self) -> Result<(), String> {
        let pubkey_bytes =
            hex::decode(&self.signer_pubkey_hex).map_err(|_| "invalid signer pubkey hex")?;
        if pubkey_bytes.len() != 32 {
            return Err("invalid signer pubkey length".into());
        }
        let mut pubkey_arr = [0u8; 32];
        pubkey_arr.copy_from_slice(&pubkey_bytes);
        let verifying_key =
            VerifyingKey::from_bytes(&pubkey_arr).map_err(|_| "invalid verifying key")?;

        let recomputed_hash = payload_hash(&self.payload);
        if recomputed_hash != self.payload_hash_hex {
            return Err("payload hash mismatch".into());
        }

        let sig_bytes = hex::decode(&self.signature_hex).map_err(|_| "invalid signature hex")?;
        if sig_bytes.len() != 64 {
            return Err("invalid signature length".into());
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
            .map_err(|_| "signature verification failed".to_string())
    }
}

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

#[cfg(test)]
mod tests {
    use super::SignedEnvelope;
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
        let payload = serde_json::json!({"job_id":"abc","tokens":8});
        let envelope = SignedEnvelope::sign(payload, &signing, 1, 1234);
        assert!(envelope.verify().is_ok());
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let signing = key();
        let payload = serde_json::json!({"job_id":"abc","tokens":8});
        let mut envelope = SignedEnvelope::sign(payload, &signing, 1, 1234);
        envelope.payload = serde_json::json!({"job_id":"abc","tokens":9});
        assert_eq!(
            envelope.verify().expect_err("tamper should fail"),
            "payload hash mismatch"
        );
    }
}
