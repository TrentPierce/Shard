use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfComputeReceipt {
    pub receipt_id: String,
    pub work_id: String,
    pub scout_id: String,
    pub verifier_id: String,
    pub token_count: u32,
    pub timestamp_ms: u128,
    pub verifier_signature_hex: String,
}

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
    pub receipt_count: usize,
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
    pub receipts: Vec<ProofOfComputeReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerSnapshot {
    pub balances: HashMap<String, i64>,
    pub tx_log: Vec<ComputeCreditTx>,
    pub receipts: Vec<ProofOfComputeReceipt>,
    pub seen: HashSet<String>,
    pub last_nonce_by_signer: HashMap<String, u64>,
    pub tx_chain_hash: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LedgerState {
    balances: HashMap<String, i64>,
    tx_log: Vec<ComputeCreditTx>,
    receipts: Vec<ProofOfComputeReceipt>,
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

    pub fn receipts(&self) -> &[ProofOfComputeReceipt] {
        &self.receipts
    }

    pub fn head(&self) -> LedgerHead {
        LedgerHead {
            height: (self.tx_log.len() + self.receipts.len()) as u64,
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
            receipt_count: self.receipts.len(),
            unique_signers: signers.len(),
            seen_tx_count: self.seen.len(),
            head: self.head(),
        }
    }

    pub fn export_range(&self, from_height: u64, limit: usize) -> LedgerExport {
        let start = from_height.saturating_sub(1) as usize;
        if start >= (self.tx_log.len() + self.receipts.len()) || limit == 0 {
            return LedgerExport {
                from_height,
                end_height: from_height.saturating_sub(1),
                has_more: false,
                txs: Vec::new(),
                receipts: Vec::new(),
            };
        }
        
        // Very basic export for now
        let end_exclusive = (start + limit).min(self.tx_log.len());
        let txs = self.tx_log[start..end_exclusive].to_vec();
        
        LedgerExport {
            from_height,
            end_height: end_exclusive as u64,
            has_more: end_exclusive < self.tx_log.len(),
            txs,
            receipts: Vec::new(),
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

    pub fn sign_poc_receipt(
        signing_key: &SigningKey,
        work_id: &str,
        scout_id: &str,
        token_count: u32,
        timestamp_ms: u128,
    ) -> ProofOfComputeReceipt {
        let verifier_id = hex::encode(signing_key.verifying_key().to_bytes());
        let receipt_id = format!("poc-{}-{}", work_id, verifier_id);
        let body = format!("poc|{}|{}|{}|{}|{}", receipt_id, work_id, scout_id, token_count, timestamp_ms);
        let sig = signing_key.sign(body.as_bytes());
        ProofOfComputeReceipt {
            receipt_id,
            work_id: work_id.to_string(),
            scout_id: scout_id.to_string(),
            verifier_id,
            token_count,
            timestamp_ms,
            verifier_signature_hex: hex::encode(sig.to_bytes()),
        }
    }

    pub fn apply_poc_receipt(&mut self, receipt: ProofOfComputeReceipt) -> Result<(), String> {
        if self.seen.contains(&receipt.receipt_id) {
            return Ok(());
        }

        let pubkey_bytes = hex::decode(&receipt.verifier_id).map_err(|_| "invalid verifier id")?;
        let verifier_key = VerifyingKey::from_bytes(
            pubkey_bytes.as_slice().try_into().map_err(|_| "invalid pubkey length")?
        ).map_err(|_| "invalid verifier key")?;
        
        let sig_bytes = hex::decode(&receipt.verifier_signature_hex).map_err(|_| "invalid signature hex")?;
        let signature = Signature::from_bytes(
            sig_bytes.as_slice().try_into().map_err(|_| "invalid signature length")?
        );

        let body = format!("poc|{}|{}|{}|{}|{}", receipt.receipt_id, receipt.work_id, receipt.scout_id, receipt.token_count, receipt.timestamp_ms);
        verifier_key.verify(body.as_bytes(), &signature).map_err(|_| "PoC receipt signature invalid")?;

        *self.balances.entry(receipt.scout_id.clone()).or_insert(0) += receipt.token_count as i64;
        self.seen.insert(receipt.receipt_id.clone());
        self.receipts.push(receipt);
        Ok(())
    }

    pub fn to_snapshot(&self) -> LedgerSnapshot {
        LedgerSnapshot {
            balances: self.balances.clone(),
            tx_log: self.tx_log.clone(),
            receipts: self.receipts.clone(),
            seen: self.seen.clone(),
            last_nonce_by_signer: self.last_nonce_by_signer.clone(),
            tx_chain_hash: self.tx_chain_hash(),
        }
    }

    pub fn from_snapshot(snapshot: LedgerSnapshot) -> Self {
        Self {
            balances: snapshot.balances,
            tx_log: snapshot.tx_log,
            receipts: snapshot.receipts,
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
        let pubkey_bytes = hex::decode(&tx.signer_pubkey_hex).map_err(|_| "invalid signer pubkey hex")?;
        let vk = VerifyingKey::from_bytes(pubkey_bytes.as_slice().try_into().map_err(|_| "invalid pubkey length")?).map_err(|_| "invalid verifying key")?;

        let sig_bytes = hex::decode(&tx.signature_hex).map_err(|_| "invalid signature hex")?;
        let sig = Signature::from_bytes(sig_bytes.as_slice().try_into().map_err(|_| "invalid signature length")?);

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
        vk.verify(body.as_bytes(), &sig).map_err(|_| "signature verification failed")?;

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
        self.last_nonce_by_signer.insert(tx.signer_pubkey_hex.clone(), tx.nonce);
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
