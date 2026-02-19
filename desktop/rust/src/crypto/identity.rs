use anyhow::{anyhow, Result};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct NodeIdentity {
    signing_key: SigningKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedIdentity {
    secret_key_hex: String,
}

impl NodeIdentity {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if let Ok(raw) = std::fs::read(path) {
            let parsed: PersistedIdentity = serde_json::from_slice(&raw)?;
            let secret = hex::decode(parsed.secret_key_hex)?;
            if secret.len() != 32 {
                return Err(anyhow!("invalid persisted ed25519 secret length"));
            }
            let mut sk = [0u8; 32];
            sk.copy_from_slice(&secret);
            return Ok(Self {
                signing_key: SigningKey::from_bytes(&sk),
            });
        }

        let mut sk = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut sk);
        let identity = Self {
            signing_key: SigningKey::from_bytes(&sk),
        };
        let persisted = PersistedIdentity {
            secret_key_hex: hex::encode(identity.signing_key.to_bytes()),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&persisted)?)?;
        Ok(identity)
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn wallet_address(&self) -> String {
        hex::encode(self.verifying_key().to_bytes())
    }

    pub fn libp2p_keypair(&self) -> Result<libp2p::identity::Keypair> {
        let mut secret = self.signing_key.to_bytes().to_vec();
        libp2p::identity::Keypair::ed25519_from_bytes(&mut secret)
            .map_err(|e| anyhow!("failed to convert identity to libp2p keypair: {e}"))
    }
}
