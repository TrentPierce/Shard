use crate::ledger::state::{ComputeCreditTx, LedgerHead, LedgerSnapshot, LedgerState};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const LEDGER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerMeta {
    pub schema_version: u32,
    pub wal_bytes: u64,
    pub height: u64,
    pub tx_chain_hash: String,
    pub state_hash: String,
}

#[derive(Debug, Clone)]
pub struct LedgerStore {
    wal_path: PathBuf,
    snapshot_path: PathBuf,
    meta_path: PathBuf,
}

impl LedgerStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            wal_path: data_dir.join("ledger.wal"),
            snapshot_path: data_dir.join("ledger.snapshot.json"),
            meta_path: data_dir.join("ledger.meta.json"),
        }
    }

    pub fn load_or_init(&self) -> Result<LedgerState> {
        let mut ledger = if self.snapshot_path.exists() {
            self.load_snapshot().unwrap_or_default()
        } else {
            LedgerState::default()
        };
        self.replay_wal(&mut ledger)?;
        self.write_meta(ledger.head())?;
        Ok(ledger)
    }

    pub fn append_tx(&self, tx: &ComputeCreditTx, head: LedgerHead) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)?;
        serde_json::to_writer(&mut file, tx)?;
        file.write_all(b"\n")?;
        file.flush()?;
        self.write_meta(head)?;
        Ok(())
    }

    pub fn write_snapshot(&self, ledger: &LedgerState) -> Result<()> {
        let snapshot = ledger.to_snapshot();
        std::fs::write(&self.snapshot_path, serde_json::to_vec_pretty(&snapshot)?)?;
        let wal_bytes = if self.wal_path.exists() {
            std::fs::metadata(&self.wal_path)?.len()
        } else {
            0
        };
        let head = ledger.head();
        let meta = LedgerMeta {
            schema_version: LEDGER_SCHEMA_VERSION,
            wal_bytes,
            height: head.height,
            tx_chain_hash: head.tx_chain_hash,
            state_hash: head.state_hash,
        };
        std::fs::write(&self.meta_path, serde_json::to_vec_pretty(&meta)?)?;
        Ok(())
    }

    fn load_snapshot(&self) -> Result<LedgerState> {
        let raw = std::fs::read(&self.snapshot_path)?;
        let snapshot: LedgerSnapshot = serde_json::from_slice(&raw)?;
        Ok(LedgerState::from_snapshot(snapshot))
    }

    fn replay_wal(&self, ledger: &mut LedgerState) -> Result<()> {
        if !self.wal_path.exists() {
            return Ok(());
        }
        let file = OpenOptions::new().read(true).open(&self.wal_path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut last_good_pos = 0u64;
        loop {
            line.clear();
            let bytes = reader.read_line(&mut line)?;
            if bytes == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                last_good_pos += bytes as u64;
                continue;
            }
            match serde_json::from_str::<ComputeCreditTx>(trimmed) {
                Ok(tx) => {
                    let _ = ledger.replay_signed_tx(tx);
                    last_good_pos += bytes as u64;
                }
                Err(_) => {
                    // Truncated/corrupt tail: truncate to last valid byte and stop.
                    let mut f = OpenOptions::new().write(true).open(&self.wal_path)?;
                    f.set_len(last_good_pos)?;
                    f.seek(SeekFrom::End(0))?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn write_meta(&self, head: LedgerHead) -> Result<()> {
        let wal_bytes = if self.wal_path.exists() {
            std::fs::metadata(&self.wal_path)?.len()
        } else {
            0
        };
        let meta = LedgerMeta {
            schema_version: LEDGER_SCHEMA_VERSION,
            wal_bytes,
            height: head.height,
            tx_chain_hash: head.tx_chain_hash,
            state_hash: head.state_hash,
        };
        std::fs::write(&self.meta_path, serde_json::to_vec_pretty(&meta)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::LedgerStore;
    use crate::ledger::state::LedgerState;
    use ed25519_dalek::SigningKey;
    use rand::RngCore;

    fn key() -> SigningKey {
        let mut sk = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut sk);
        SigningKey::from_bytes(&sk)
    }

    #[test]
    fn wal_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = LedgerStore::new(dir.path());
        let mut ledger = LedgerState::default();
        let signing = key();
        let tx =
            LedgerState::sign_reward_tx(&signing, "wallet-a", "wallet-b", 2, "r1", "s1", 1, 100);
        ledger.apply_signed_tx(tx.clone()).expect("apply");
        store.append_tx(&tx, ledger.head()).expect("append");
        store.write_snapshot(&ledger).expect("snapshot");

        let recovered = store.load_or_init().expect("load");
        assert_eq!(recovered.balance_of("wallet-b"), 2);
        assert_eq!(recovered.head().height, 1);
    }
}
