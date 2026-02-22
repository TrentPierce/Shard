use shard_common::common::signed_envelope::{EnvelopeVerifier, SignedEnvelope};
use shard_common::types::WorkResponse;
use serde_json;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Error type for draft verification
#[derive(Debug)]
pub enum VerificationError {
    InvalidJson(String),
    SignatureInvalid(String),
}

/// Enforces cryptographic verification on draft submissions from Scouts.
pub async fn verify_draft_submission(
    payload: &[u8],
    verifier: Arc<Mutex<EnvelopeVerifier>>,
) -> Result<WorkResponse, VerificationError> {
    // Attempt to parse as SignedEnvelope<WorkResponse>
    let envelope: SignedEnvelope<WorkResponse> = serde_json::from_slice(payload)
        .map_err(|e| VerificationError::InvalidJson(e.to_string()))?;

    // Cryptographically verify the signature and replay resistance
    let mut verifier_guard = verifier.lock().await;
    match verifier_guard.verify_full(&envelope) {
        Ok(_) => Ok(envelope.payload),
        Err(e) => Err(VerificationError::SignatureInvalid(e.to_string())),
    }
}
