use crate::crypto::identity::NodeIdentity;
use anyhow::{anyhow, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;

const WALLET_BACKUP_VERSION: u32 = 1;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const DERIVED_KEY_BYTES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBackupEnvelope {
    pub version: u32,
    pub kdf: KdfConfig,
    pub cipher: String,
    pub nonce: String,
    pub ciphertext: String,
    pub public_key_hex: String,
    pub created_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfConfig {
    pub algorithm: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalletBackupPayload {
    secret_key_hex: String,
    public_key_hex: String,
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn derive_key(password: &str, cfg: &KdfConfig) -> Result<[u8; DERIVED_KEY_BYTES]> {
    if cfg.algorithm != "argon2id" {
        return Err(anyhow!("unsupported KDF algorithm: {}", cfg.algorithm));
    }
    let salt = hex::decode(&cfg.salt).map_err(|e| anyhow!("invalid salt hex: {e}"))?;
    let params = Params::new(
        cfg.memory_kib,
        cfg.iterations,
        cfg.parallelism,
        Some(DERIVED_KEY_BYTES),
    )
    .map_err(|e| anyhow!("invalid argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; DERIVED_KEY_BYTES];
    argon2
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .map_err(|e| anyhow!("argon2 derivation failed: {e}"))?;
    Ok(key)
}

pub fn export_wallet(
    identity_path: &Path,
    out_path: &Path,
    password: &str,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<String> {
    let identity = NodeIdentity::load(identity_path)?;
    let public_key_hex = identity.wallet_address();
    let payload = WalletBackupPayload {
        secret_key_hex: hex::encode(identity.secret_bytes()),
        public_key_hex: public_key_hex.clone(),
    };
    let payload_bytes = serde_json::to_vec(&payload)?;

    let mut salt = [0u8; SALT_BYTES];
    rand::thread_rng().fill_bytes(&mut salt);
    let kdf = KdfConfig {
        algorithm: "argon2id".into(),
        memory_kib,
        iterations,
        parallelism,
        salt: hex::encode(salt),
    };
    let key = derive_key(password, &kdf)?;

    let mut nonce = [0u8; NONCE_BYTES];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), payload_bytes.as_ref())
        .map_err(|e| anyhow!("wallet backup encryption failed: {e}"))?;

    let envelope = WalletBackupEnvelope {
        version: WALLET_BACKUP_VERSION,
        kdf,
        cipher: "xchacha20poly1305".into(),
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ciphertext),
        public_key_hex: public_key_hex.clone(),
        created_at_ms: now_ms(),
    };
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, serde_json::to_vec_pretty(&envelope)?)?;
    Ok(public_key_hex)
}

pub fn verify_backup(backup_path: &Path, password: &str) -> Result<String> {
    let raw = std::fs::read(backup_path)?;
    let envelope: WalletBackupEnvelope = serde_json::from_slice(&raw)?;
    if envelope.version != WALLET_BACKUP_VERSION {
        return Err(anyhow!(
            "unsupported wallet backup version {}",
            envelope.version
        ));
    }
    if envelope.cipher != "xchacha20poly1305" {
        return Err(anyhow!(
            "unsupported wallet backup cipher {}",
            envelope.cipher
        ));
    }
    let key = derive_key(password, &envelope.kdf)?;
    let nonce_raw = hex::decode(&envelope.nonce).map_err(|e| anyhow!("invalid nonce hex: {e}"))?;
    if nonce_raw.len() != NONCE_BYTES {
        return Err(anyhow!("invalid backup nonce length"));
    }
    let ciphertext =
        hex::decode(&envelope.ciphertext).map_err(|e| anyhow!("invalid ciphertext hex: {e}"))?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let plaintext = cipher
        .decrypt(XNonce::from_slice(&nonce_raw), ciphertext.as_ref())
        .map_err(|_| {
            anyhow!("wallet backup decryption failed (wrong password or corrupted file)")
        })?;
    let payload: WalletBackupPayload = serde_json::from_slice(&plaintext)?;
    let secret_raw = hex::decode(&payload.secret_key_hex)
        .map_err(|e| anyhow!("invalid secret key in backup payload: {e}"))?;
    if secret_raw.len() != 32 {
        return Err(anyhow!("invalid secret key length in backup payload"));
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&secret_raw);
    let identity = NodeIdentity::from_secret_bytes(secret);
    let derived_public = identity.wallet_address();
    if derived_public != payload.public_key_hex || derived_public != envelope.public_key_hex {
        return Err(anyhow!("wallet backup pubkey mismatch"));
    }
    Ok(derived_public)
}

pub fn import_wallet(
    backup_path: &Path,
    identity_path: &Path,
    password: &str,
    force: bool,
) -> Result<String> {
    if identity_path.exists() && !force {
        return Err(anyhow!(
            "identity file already exists at {} (use --force to overwrite)",
            identity_path.display()
        ));
    }

    let raw = std::fs::read(backup_path)?;
    let envelope: WalletBackupEnvelope = serde_json::from_slice(&raw)?;
    let key = derive_key(password, &envelope.kdf)?;
    let nonce_raw = hex::decode(&envelope.nonce).map_err(|e| anyhow!("invalid nonce hex: {e}"))?;
    if nonce_raw.len() != NONCE_BYTES {
        return Err(anyhow!("invalid backup nonce length"));
    }
    let ciphertext =
        hex::decode(&envelope.ciphertext).map_err(|e| anyhow!("invalid ciphertext hex: {e}"))?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let plaintext = cipher
        .decrypt(XNonce::from_slice(&nonce_raw), ciphertext.as_ref())
        .map_err(|_| {
            anyhow!("wallet backup decryption failed (wrong password or corrupted file)")
        })?;
    let payload: WalletBackupPayload = serde_json::from_slice(&plaintext)?;

    let secret_raw = hex::decode(&payload.secret_key_hex)
        .map_err(|e| anyhow!("invalid secret key in backup payload: {e}"))?;
    if secret_raw.len() != 32 {
        return Err(anyhow!("invalid secret key length in backup payload"));
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&secret_raw);
    let identity = NodeIdentity::from_secret_bytes(secret);
    let public = identity.wallet_address();
    if public != payload.public_key_hex || public != envelope.public_key_hex {
        return Err(anyhow!("wallet backup pubkey mismatch"));
    }
    identity.save(identity_path)?;
    Ok(public)
}

#[cfg(test)]
mod tests {
    use super::{export_wallet, import_wallet, verify_backup};
    use crate::crypto::identity::NodeIdentity;

    #[test]
    fn wallet_backup_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let identity_path = dir.path().join("identity.json");
        let backup_path = dir.path().join("wallet.shard-wallet");

        let original = NodeIdentity::load_or_create(&identity_path).expect("identity");
        let original_wallet = original.wallet_address();
        let exported =
            export_wallet(&identity_path, &backup_path, "pw", 64 * 1024, 3, 1).expect("export");
        assert_eq!(exported, original_wallet);

        let verified = verify_backup(&backup_path, "pw").expect("verify");
        assert_eq!(verified, original_wallet);

        std::fs::remove_file(&identity_path).expect("remove old identity");
        let imported = import_wallet(&backup_path, &identity_path, "pw", false).expect("import");
        assert_eq!(imported, original_wallet);
        let imported_wallet = NodeIdentity::load(&identity_path)
            .expect("load imported")
            .wallet_address();
        assert_eq!(imported_wallet, original_wallet);
    }
}
