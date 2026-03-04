use anyhow::Result;
use futures::StreamExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone, serde::Deserialize)]
struct ModelManifest {
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

    let resp = reqwest::get(manifest_url().as_str()).await?;
    if !resp.status().is_success() {
        anyhow::bail!("manifest fetch failed: {}", resp.status());
    }
    let manifest: ModelManifest = resp.json().await?;

    let entry = manifest
        .models
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("model manifest is empty"))?;

    // Detect placeholder URL (manifest not yet configured with real URLs).
    if entry.download_url.starts_with("REPLACE_")
        || entry.download_url.is_empty()
        || !entry.download_url.starts_with("http")
    {
        anyhow::bail!(
            "Model download URL not configured yet. Add your BitNet model path manually in Settings."
        );
    }

    let final_path = model_path(&data_dir, &entry);

    // Already present — skip download.
    if final_path.exists() {
        return Ok(final_path);
    }

    std::fs::create_dir_all(final_path.parent().unwrap())?;
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
    Ok(final_path)
}
