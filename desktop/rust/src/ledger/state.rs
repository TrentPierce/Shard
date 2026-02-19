use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeCreditTx {
    pub tx_id: String,
    pub from_wallet: String,
    pub to_wallet: String,
    pub amount: i64,
    pub request_id: String,
    pub step_id: String,
    pub nonce: u64,
    pub created_at_ms: u128,
    pub signer_pubkey_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LedgerState {
    balances: HashMap<String, i64>,
    tx_log: Vec<ComputeCreditTx>,
    seen: HashSet<String>,
}

impl LedgerState {
    pub fn balance_of(&self, wallet: &str) -> i64 {
        self.balances.get(wallet).copied().unwrap_or(0)
    }

    pub fn tx_by_id(&self, tx_id: &str) -> Option<ComputeCreditTx> {
        self.tx_log.iter().find(|t| t.tx_id == tx_id).cloned()
    }

    pub fn sign_reward_tx(
        signing_key: &SigningKey,
        from_wallet: &str,
        to_wallet: &str,
        amount: i64,
        request_id: &str,
        step_id: &str,
        nonce: u64,
        created_at_ms: u128,
    ) -> ComputeCreditTx {
        let signer_pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let tx_id = format!("tx-{}-{}", request_id, nonce);
        let body = signing_payload(
            &tx_id,
            from_wallet,
            to_wallet,
            amount,
            request_id,
            step_id,
            nonce,
            created_at_ms,
            &signer_pubkey_hex,
        );
        let sig = signing_key.sign(body.as_bytes());
        ComputeCreditTx {
            tx_id,
            from_wallet: from_wallet.to_string(),
            to_wallet: to_wallet.to_string(),
            amount,
            request_id: request_id.to_string(),
            step_id: step_id.to_string(),
            nonce,
            created_at_ms,
            signer_pubkey_hex,
            signature_hex: hex::encode(sig.to_bytes()),
        }
    }

    pub fn apply_signed_tx(&mut self, tx: ComputeCreditTx) -> Result<(), String> {
        if tx.amount <= 0 {
            return Err("amount must be positive".into());
        }
        if self.seen.contains(&tx.tx_id) {
            return Ok(());
        }

        let pubkey_bytes =
            hex::decode(&tx.signer_pubkey_hex).map_err(|_| "invalid signer pubkey hex")?;
        if pubkey_bytes.len() != 32 {
            return Err("invalid signer pubkey length".into());
        }
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&pubkey_bytes);
        let vk = VerifyingKey::from_bytes(&pubkey).map_err(|_| "invalid verifying key")?;

        let sig_bytes = hex::decode(&tx.signature_hex).map_err(|_| "invalid signature hex")?;
        if sig_bytes.len() != 64 {
            return Err("invalid signature length".into());
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let sig = Signature::from_bytes(&sig_arr);

        let body = signing_payload(
            &tx.tx_id,
            &tx.from_wallet,
            &tx.to_wallet,
            tx.amount,
            &tx.request_id,
            &tx.step_id,
            tx.nonce,
            tx.created_at_ms,
            &tx.signer_pubkey_hex,
        );
        vk.verify(body.as_bytes(), &sig)
            .map_err(|_| "signature verification failed")?;

        // Keep accounting simple for phase 3: rewards mint credits to receiver
        // while sender debits are tracked for auditability.
        *self.balances.entry(tx.from_wallet.clone()).or_insert(0) -= tx.amount;
        *self.balances.entry(tx.to_wallet.clone()).or_insert(0) += tx.amount;
        self.seen.insert(tx.tx_id.clone());
        self.tx_log.push(tx);
        Ok(())
    }
}

fn signing_payload(
    tx_id: &str,
    from_wallet: &str,
    to_wallet: &str,
    amount: i64,
    request_id: &str,
    step_id: &str,
    nonce: u64,
    created_at_ms: u128,
    signer_pubkey_hex: &str,
) -> String {
    format!(
        "{tx_id}|{from_wallet}|{to_wallet}|{amount}|{request_id}|{step_id}|{nonce}|{created_at_ms}|{signer_pubkey_hex}"
    )
}

#[cfg(test)]
mod tests {
    use super::LedgerState;
    use ed25519_dalek::SigningKey;
    use rand::RngCore;

    fn key() -> SigningKey {
        let mut sk = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut sk);
        SigningKey::from_bytes(&sk)
    }

    #[test]
    fn signed_tx_applies_balance() {
        let signing = key();
        let mut ledger = LedgerState::default();
        let tx = LedgerState::sign_reward_tx(
            &signing,
            "wallet-a",
            "wallet-b",
            3,
            "r1",
            "s1",
            1,
            100,
        );
        ledger.apply_signed_tx(tx).expect("apply");
        assert_eq!(ledger.balance_of("wallet-b"), 3);
        assert_eq!(ledger.balance_of("wallet-a"), -3);
    }
}
