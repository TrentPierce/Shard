use crate::ledger::state::{ComputeCreditTx, LedgerHead};
use serde::{Deserialize, Serialize};
use sha2::Digest;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LedgerSyncRequest {
    HeadRequest,
    RangeRequest { from_height: u64, max_items: usize },
    HashProbe { from_height: u64, to_height: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LedgerSyncResponse {
    HeadResponse {
        head: LedgerHead,
    },
    RangeResponse {
        from_height: u64,
        end_height: u64,
        has_more: bool,
        txs: Vec<ComputeCreditTx>,
    },
    HashProbeResponse {
        from_height: u64,
        to_height: u64,
        segment_hashes: Vec<String>,
    },
    Error {
        detail: String,
    },
}

pub fn hash_probe_segments(
    txs: &[ComputeCreditTx],
    from_height: u64,
    to_height: u64,
    segment_size: usize,
) -> Vec<String> {
    if segment_size == 0 || to_height < from_height {
        return Vec::new();
    }
    let start = from_height.saturating_sub(1) as usize;
    let end = (to_height as usize).min(txs.len());
    if start >= end {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut idx = start;
    while idx < end {
        let seg_end = (idx + segment_size).min(end);
        let mut hasher = sha2::Sha256::new();
        for tx in &txs[idx..seg_end] {
            let bytes = serde_json::to_vec(tx).unwrap_or_default();
            hasher.update(bytes);
        }
        out.push(hex::encode(sha2::Digest::finalize(hasher)));
        idx = seg_end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::hash_probe_segments;
    use crate::ledger::state::LedgerState;
    use ed25519_dalek::SigningKey;
    use rand::RngCore;

    fn key() -> SigningKey {
        let mut sk = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut sk);
        SigningKey::from_bytes(&sk)
    }

    #[test]
    fn hash_probe_produces_segments() {
        let mut ledger = LedgerState::default();
        let signing = key();
        for i in 0..5 {
            let tx = LedgerState::sign_reward_tx(
                &signing,
                "wallet-a",
                "wallet-b",
                1,
                &format!("r{i}"),
                "s1",
                (i + 1) as u64,
                100 + i as u128,
            );
            ledger.apply_signed_tx(tx).expect("apply");
        }
        let hashes = hash_probe_segments(ledger.txs(), 1, 5, 2);
        assert_eq!(hashes.len(), 3);
    }
}
