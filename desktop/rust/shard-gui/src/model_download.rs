use anyhow::{Context, Result};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::warn;

const DEFAULT_MODEL_ID: &str = "meta-llama/Llama-3.2-1B";
const DEFAULT_MODEL_VERSION: &str = "1.0.0";
const DEFAULT_MODEL_URL: &str = "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf";
const DEFAULT_MODEL_SHA256: &str =
    "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83";
const DEFAULT_MODEL_SIZE_BYTES: u64 = 807_694_464;

#[derive(Debug, Clone, serde::Deserialize)]
struct ModelManifest {
    #[serde(default)]
    models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ModelEntry {
    id: String,
    version: String,
    #[serde(default)]
    sha256: String,
    #[serde(default)]
    size_bytes: u64,
    download_url: String,
}

#[derive(Debug, Clone)]
pub enum DownloadMsg {
    Progress {
        downloaded: u64,
        total: u64,
        filename: String,
    },
    Done(PathBuf),
    Error(String),
}

fn manifest_url() -> String {
    std::env::var("SHARD_MODEL_MANIFEST_URL").unwrap_or_else(|_| {
        "https://raw.githubusercontent.com/TrentPierce/Shard/main/deploy/models/manifest.json"
            .to_string()
    })
}

fn default_model_entry() -> ModelEntry {
    let size_bytes = std::env::var("SHARD_DEFAULT_MODEL_SIZE_BYTES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_MODEL_SIZE_BYTES);
    ModelEntry {
        id: std::env::var("SHARD_DEFAULT_MODEL_ID")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL_ID.to_string()),
        version: std::env::var("SHARD_DEFAULT_MODEL_VERSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL_VERSION.to_string()),
        sha256: std::env::var("SHARD_DEFAULT_MODEL_SHA256")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL_SHA256.to_string()),
        size_bytes,
        download_url: std::env::var("SHARD_DEFAULT_MODEL_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL_URL.to_string()),
    }
}

fn has_valid_download_url(url: &str) -> bool {
    let u = url.trim();
    !u.is_empty()
        && !u.starts_with("REPLACE_")
        && (u.starts_with("http://") || u.starts_with("https://"))
}

fn normalized_sha256(input: &str) -> Option<String> {
    let s = input.trim().to_ascii_lowercase();
    if s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(s)
    } else {
        None
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to open file for sha256: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn fetch_manifest_entry() -> Result<Option<ModelEntry>> {
    let resp = reqwest::get(manifest_url().as_str()).await?;
    if !resp.status().is_success() {
        anyhow::bail!("manifest fetch failed: {}", resp.status());
    }
    let manifest: ModelManifest = resp.json().await?;
    Ok(manifest
        .models
        .into_iter()
        .find(|entry| has_valid_download_url(entry.download_url.as_str())))
}

pub fn daemon_data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("SHARD_DATA_DIR") {
        let t = d.trim().to_string();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("shard")
}

fn model_path(data_dir: &Path, entry: &ModelEntry) -> PathBuf {
    let filename = entry
        .download_url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("model.bin");
    data_dir
        .join("models")
        .join(&entry.id)
        .join(&entry.version)
        .join(filename)
}

/// Fetch manifest, pick the default model, download with progress, send Done/Error.
pub async fn run_download(tx: mpsc::Sender<DownloadMsg>) {
    match download_inner(&tx).await {
        Ok(path) => {
            let _ = tx.send(DownloadMsg::Done(path)).await;
        }
        Err(e) => {
            let _ = tx.send(DownloadMsg::Error(e.to_string())).await;
        }
    }
}

async fn download_inner(tx: &mpsc::Sender<DownloadMsg>) -> Result<PathBuf> {
    let data_dir = daemon_data_dir();
    let entry = match fetch_manifest_entry().await {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            warn!("model manifest had no valid download entries; using built-in default model source");
            default_model_entry()
        }
        Err(err) => {
            warn!(error = %err, "model manifest fetch failed; using built-in default model source");
            default_model_entry()
        }
    };

    if !has_valid_download_url(entry.download_url.as_str()) {
        anyhow::bail!("model download URL is invalid and no valid fallback URL is configured");
    }

    let expected_sha256 = normalized_sha256(entry.sha256.as_str());
    let final_path = model_path(&data_dir, &entry);

    // Already present - verify when hash is available, otherwise skip download.
    if final_path.exists() {
        if let Some(expected) = expected_sha256.as_ref() {
            let got = sha256_file(final_path.as_path())?;
            if got.eq_ignore_ascii_case(expected.as_str()) {
                return Ok(final_path);
            }
            warn!(
                path = %final_path.display(),
                "existing model checksum mismatch; re-downloading"
            );
            let _ = std::fs::remove_file(final_path.as_path());
        } else {
            return Ok(final_path);
        }
    }

    std::fs::create_dir_all(
        final_path
            .parent()
            .context("resolved model path has no parent directory")?,
    )?;
    let tmp = final_path.with_extension("download");

    let filename = final_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let client = reqwest::Client::new();
    let resp = client.get(&entry.download_url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("model download failed: {}", resp.status());
    }
    let total = resp.content_length().unwrap_or(entry.size_bytes);
    let mut downloaded = 0u64;

    let mut stream = resp.bytes_stream();
    let mut file = tokio::io::BufWriter::new(tokio::fs::File::create(&tmp).await?);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as u64;
        file.write_all(&chunk).await?;
        let _ = tx.try_send(DownloadMsg::Progress {
            downloaded,
            total,
            filename: filename.clone(),
        });
    }
    file.flush().await?;
    drop(file);

    tokio::fs::rename(&tmp, &final_path).await?;

    if let Some(expected) = expected_sha256.as_ref() {
        let got = sha256_file(final_path.as_path())?;
        if !got.eq_ignore_ascii_case(expected.as_str()) {
            let _ = tokio::fs::remove_file(final_path.as_path()).await;
            anyhow::bail!(
                "downloaded model checksum mismatch (expected {}, got {})",
                expected,
                got
            );
        }
    }

    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::{has_valid_download_url, normalized_sha256};
    use anyhow::Result;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use tokio::sync::mpsc;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.previous.as_ref() {
                std::env::set_var(self.key, prev);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time ok")
            .as_nanos();
        dir.push(format!("shard-gui-{label}-{}-{ts}", std::process::id()));
        dir
    }

    fn spawn_single_download_server(payload: Vec<u8>) -> Result<(String, thread::JoinHandle<()>)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut req_buf = [0u8; 4096];
                let _ = stream.read(&mut req_buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(payload.as_slice());
                let _ = stream.flush();
            }
        });
        Ok((format!("http://{addr}/model.gguf"), handle))
    }

    #[test]
    fn normalized_sha256_accepts_lower_or_upper_hex() {
        let input = "6F85A640A97CF2BF5B8E764087B1E83DA0FDB51D7C9FAB7D0FECE9385611DF83";
        let parsed = normalized_sha256(input).expect("valid sha");
        assert_eq!(
            parsed,
            "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83"
        );
    }

    #[test]
    fn normalized_sha256_rejects_placeholders() {
        assert!(normalized_sha256("REPLACE_WITH_ACTUAL_HASH").is_none());
        assert!(normalized_sha256("").is_none());
    }

    #[test]
    fn valid_download_url_filters_placeholders() {
        assert!(has_valid_download_url("https://example.com/model.gguf"));
        assert!(!has_valid_download_url("REPLACE_WITH_ACTUAL_URL"));
        assert!(!has_valid_download_url(""));
        assert!(!has_valid_download_url("ftp://example.com/model.gguf"));
    }

    #[tokio::test]
    async fn download_inner_falls_back_and_verifies_hash() -> Result<()> {
        let _lock = env_lock().lock().expect("env lock");

        let payload = b"smoke-model-payload".to_vec();
        let expected_sha = format!("{:x}", Sha256::digest(payload.as_slice()));
        let (download_url, server) = spawn_single_download_server(payload.clone())?;

        let data_dir = unique_temp_dir("download-fallback");
        std::fs::create_dir_all(data_dir.as_path())?;

        let _g1 = EnvVarGuard::set("SHARD_DATA_DIR", data_dir.to_string_lossy().as_ref());
        let _g2 = EnvVarGuard::set("SHARD_MODEL_MANIFEST_URL", "http://127.0.0.1:9/manifest.json");
        let _g3 = EnvVarGuard::set("SHARD_DEFAULT_MODEL_ID", "smoke-model");
        let _g4 = EnvVarGuard::set("SHARD_DEFAULT_MODEL_VERSION", "test");
        let _g5 = EnvVarGuard::set("SHARD_DEFAULT_MODEL_URL", download_url.as_str());
        let _g6 = EnvVarGuard::set("SHARD_DEFAULT_MODEL_SHA256", expected_sha.as_str());
        let _g7 = EnvVarGuard::set(
            "SHARD_DEFAULT_MODEL_SIZE_BYTES",
            payload.len().to_string().as_str(),
        );

        let (tx, _rx) = mpsc::channel(4);
        let path = super::download_inner(&tx).await?;
        let got = std::fs::read(path.as_path())?;
        assert_eq!(got, payload);
        assert!(path
            .to_string_lossy()
            .contains("smoke-model\\test\\model.gguf"));

        server.join().expect("download server thread");
        let _ = std::fs::remove_dir_all(data_dir);
        Ok(())
    }
}
