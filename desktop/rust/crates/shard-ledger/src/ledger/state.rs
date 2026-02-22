use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerHead {
    pub height: u64,
    pub tx_chain_hash: String,
    pub state_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerStats {
    pub wallet_count: usize,
    pub tx_count: usize,
    pub unique_signers: usize,
    pub seen_tx_count: usize,
    pub head: LedgerHead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerExport {
    pub from_height: u64,
    pub end_height: u64,
    pub has_more: bool,
    pub txs: Vec<ComputeCreditTx>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerSnapshot {
    pub balances: HashMap<String, i64>,
    pub tx_log: Vec<ComputeCreditTx>,
    pub seen: HashSet<String>,
    pub last_nonce_by_signer: HashMap<String, u64>,
    pub tx_chain_hash: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LedgerState {
    balances: HashMap<String, i64>,
    tx_log: Vec<ComputeCreditTx>,
    seen: HashSet<String>,
    last_nonce_by_signer: HashMap<String, u64>,
    tx_chain_hash: String,
}

impl LedgerState {
    pub fn balance_of(&self, wallet: &str) -> i64 {
        self.balances.get(wallet).copied().unwrap_or(0)
    }

    pub fn tx_by_id(&self, tx_id: &str) -> Option<ComputeCreditTx> {
        self.tx_log.iter().find(|t| t.tx_id == tx_id).cloned()
    }

    pub fn txs(&self) -> &[ComputeCreditTx] {
        &self.tx_log
    }

    pub fn head(&self) -> LedgerHead {
        LedgerHead {
            height: self.tx_log.len() as u64,
            tx_chain_hash: self.tx_chain_hash(),
            state_hash: self.state_hash(),
        }
    }

    pub fn stats(&self) -> LedgerStats {
        let mut signers = HashSet::new();
        for tx in &self.tx_log {
            signers.insert(tx.signer_pubkey_hex.clone());
        }
        LedgerStats {
            wallet_count: self.balances.len(),
            tx_count: self.tx_log.len(),
            unique_signers: signers.len(),
            seen_tx_count: self.seen.len(),
            head: self.head(),
        }
    }

    pub fn export_range(&self, from_height: u64, limit: usize) -> LedgerExport {
        let start = from_height.saturating_sub(1) as usize;
        if start >= self.tx_log.len() || limit == 0 {
            return LedgerExport {
                from_height,
                end_height: from_height.saturating_sub(1),
                has_more: false,
                txs: Vec::new(),
            };
        }
        let end_exclusive = (start + limit).min(self.tx_log.len());
        let txs = self.tx_log[start..end_exclusive].to_vec();
        let end_height = end_exclusive as u64;
        LedgerExport {
            from_height,
            end_height,
            has_more: end_exclusive < self.tx_log.len(),
            txs,
        }
    }

    pub fn tx_chain_hash(&self) -> String {
        if self.tx_chain_hash.is_empty() {
            "0".repeat(64)
        } else {
            self.tx_chain_hash.clone()
        }
    }

    pub fn state_hash(&self) -> String {
        let mut balance_items: Vec<(&String, &i64)> = self.balances.iter().collect();
        balance_items.sort_by(|a, b| a.0.cmp(b.0));

        let mut hasher = Sha256::new();
        hasher.update((self.tx_log.len() as u64).to_le_bytes());
        hasher.update(self.tx_chain_hash().as_bytes());
        for (wallet, amount) in balance_items {
            hasher.update(wallet.as_bytes());
            hasher.update(amount.to_le_bytes());
        }
        hex::encode(hasher.finalize())
    }

    pub fn to_snapshot(&self) -> LedgerSnapshot {
        LedgerSnapshot {
            balances: self.balances.clone(),
            tx_log: self.tx_log.clone(),
            seen: self.seen.clone(),
            last_nonce_by_signer: self.last_nonce_by_signer.clone(),
            tx_chain_hash: self.tx_chain_hash(),
        }
    }

    pub fn from_snapshot(snapshot: LedgerSnapshot) -> Self {
        Self {
            balances: snapshot.balances,
            tx_log: snapshot.tx_log,
            seen: snapshot.seen,
            last_nonce_by_signer: snapshot.last_nonce_by_signer,
            tx_chain_hash: snapshot.tx_chain_hash,
        }
    }

    #[allow(clippy::too_many_arguments)]
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
        self.validate_tx(&tx)?;
        if self.seen.contains(&tx.tx_id) {
            return Ok(());
        }
        self.apply_verified_tx(tx);
        Ok(())
    }

    pub fn replay_signed_tx(&mut self, tx: ComputeCreditTx) -> Result<(), String> {
        self.validate_tx(&tx)?;
        if self.seen.contains(&tx.tx_id) {
            return Ok(());
        }
        self.apply_verified_tx(tx);
        Ok(())
    }

    fn validate_tx(&self, tx: &ComputeCreditTx) -> Result<(), String> {
        if tx.amount <= 0 {
            return Err("amount must be positive".into());
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

        if let Some(prev_nonce) = self.last_nonce_by_signer.get(&tx.signer_pubkey_hex) {
            if tx.nonce <= *prev_nonce {
                return Err("stale signer nonce".into());
            }
        }
        Ok(())
    }

    fn apply_verified_tx(&mut self, tx: ComputeCreditTx) {
        *self.balances.entry(tx.from_wallet.clone()).or_insert(0) -= tx.amount;
        *self.balances.entry(tx.to_wallet.clone()).or_insert(0) += tx.amount;
        self.last_nonce_by_signer
            .insert(tx.signer_pubkey_hex.clone(), tx.nonce);
        self.seen.insert(tx.tx_id.clone());
        self.roll_tx_chain_hash(&tx);
        self.tx_log.push(tx);
    }

    fn roll_tx_chain_hash(&mut self, tx: &ComputeCreditTx) {
        let mut hasher = Sha256::new();
        hasher.update(self.tx_chain_hash().as_bytes());
        hasher.update(canonical_tx_bytes(tx));
        self.tx_chain_hash = hex::encode(hasher.finalize());
    }
}

fn canonical_tx_bytes(tx: &ComputeCreditTx) -> Vec<u8> {
    serde_json::to_vec(tx).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
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
        let tx =
            LedgerState::sign_reward_tx(&signing, "wallet-a", "wallet-b", 3, "r1", "s1", 1, 100);
        ledger.apply_signed_tx(tx).expect("apply");
        assert_eq!(ledger.balance_of("wallet-b"), 3);
        assert_eq!(ledger.balance_of("wallet-a"), -3);
    }

    #[test]
    fn stale_nonce_is_rejected() {
        let signing = key();
        let signer_pub = hex::encode(signing.verifying_key().to_bytes());
        let mut ledger = LedgerState::default();
        let tx1 =
            LedgerState::sign_reward_tx(&signing, "wallet-a", "wallet-b", 1, "r1", "s1", 10, 100);
        ledger.apply_signed_tx(tx1).expect("tx1");
        let tx2 =
            LedgerState::sign_reward_tx(&signing, "wallet-a", "wallet-c", 1, "r2", "s1", 10, 101);
        assert_eq!(
            ledger.apply_signed_tx(tx2).expect_err("reject stale"),
            "stale signer nonce"
        );
        assert!(ledger.stats().unique_signers >= 1);
        assert_eq!(ledger.head().height, 1);
        assert!(ledger.stats().head.tx_chain_hash.len() == 64);
        assert!(ledger.stats().head.state_hash.len() == 64);
        assert!(ledger.txs()[0].signer_pubkey_hex == signer_pub);
    }
}
