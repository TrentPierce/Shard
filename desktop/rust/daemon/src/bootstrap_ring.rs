use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BootstrapPeer {
    pub region: String,
    pub label: String,
    pub addr: String,
    pub health_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BootstrapRingConfig {
    bootstrap_peers: Vec<BootstrapPeer>,
    min_connected_bootstrap: usize,
    bootstrap_retry_interval_seconds: u64,
    bootstrap_retry_max_backoff_seconds: u64,
    refuse_work_below_min_bootstrap: bool,
}

pub(crate) struct BootstrapRing {
    pub peers: Vec<BootstrapPeer>,
    pub min_connected: usize,
    pub connected: Arc<Mutex<HashSet<String>>>,
    pub retry_handles: Vec<JoinHandle<()>>,
    pub retry_interval_seconds: u64,
    pub max_backoff_seconds: u64,
    pub refuse_work_below_min_bootstrap: bool,
}

impl BootstrapRing {
    pub(crate) fn from_config(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed reading bootstrap ring config: {}", path.display()))?;
        let parsed: BootstrapRingConfig = serde_yaml::from_str(raw.as_str())
            .with_context(|| format!("failed parsing bootstrap ring config: {}", path.display()))?;
        let unique_addrs: HashSet<String> = parsed
            .bootstrap_peers
            .iter()
            .map(|peer| peer.addr.clone())
            .collect();
        if unique_addrs.is_empty() {
            anyhow::bail!("bootstrap ring config has no peers: {}", path.display());
        }
        if parsed.min_connected_bootstrap == 0 {
            anyhow::bail!(
                "bootstrap ring config min_connected_bootstrap must be >= 1: {}",
                path.display()
            );
        }
        if parsed.min_connected_bootstrap > unique_addrs.len() {
            anyhow::bail!(
                "bootstrap ring config min_connected_bootstrap={} cannot be satisfied by {} unique peer addr entries in {}",
                parsed.min_connected_bootstrap,
                unique_addrs.len(),
                path.display()
            );
        }
        Ok(Self {
            peers: parsed.bootstrap_peers,
            min_connected: parsed.min_connected_bootstrap,
            connected: Arc::new(Mutex::new(HashSet::new())),
            retry_handles: Vec::new(),
            retry_interval_seconds: parsed.bootstrap_retry_interval_seconds.max(1),
            max_backoff_seconds: parsed.bootstrap_retry_max_backoff_seconds.max(1),
            refuse_work_below_min_bootstrap: parsed.refuse_work_below_min_bootstrap,
        })
    }

    pub(crate) fn connect_all(&mut self) {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        for peer in self.peers.clone() {
            let connected = Arc::clone(&self.connected);
            let retry_initial = self.retry_interval_seconds;
            let retry_max = self.max_backoff_seconds;
            let client = client.clone();
            let handle = tokio::spawn(async move {
                let mut backoff = retry_initial;
                loop {
                    let ok = client
                        .get(peer.health_url.as_str())
                        .send()
                        .await
                        .map(|resp| resp.status().is_success())
                        .unwrap_or(false);
                    if ok {
                        {
                            let mut guard = connected.lock().await;
                            guard.insert(peer.addr.clone());
                        }
                        tracing::info!(
                            region = %peer.region,
                            label = %peer.label,
                            addr = %peer.addr,
                            "bootstrap peer healthy"
                        );
                        backoff = retry_initial;
                        sleep(Duration::from_secs(15)).await;
                        continue;
                    }

                    {
                        let mut guard = connected.lock().await;
                        guard.remove(peer.addr.as_str());
                    }
                    tracing::warn!(
                        region = %peer.region,
                        label = %peer.label,
                        addr = %peer.addr,
                        backoff_seconds = backoff,
                        "bootstrap peer health check failed"
                    );
                    sleep(Duration::from_secs(backoff)).await;
                    backoff = (backoff.saturating_mul(2)).min(retry_max);
                }
            });
            self.retry_handles.push(handle);
        }
    }

    pub(crate) async fn is_healthy(&self) -> bool {
        self.connected.lock().await.len() >= self.min_connected
    }

    pub(crate) async fn connected_count(&self) -> usize {
        self.connected.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::BootstrapRing;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_yaml_path(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("bootstrap-ring-{suffix}-{nanos}.yaml"))
    }

    #[test]
    fn rejects_unsatisfiable_min_connected_for_duplicate_addrs() {
        let path = temp_yaml_path("dup");
        let body = r#"
bootstrap_peers:
  - region: us-east-1
    label: "A"
    addr: /ip4/1.2.3.4/tcp/4001/p2p/peer
    health_url: http://1.2.3.4:9091/health
  - region: eu-west-1
    label: "B"
    addr: /ip4/1.2.3.4/tcp/4001/p2p/peer
    health_url: http://1.2.3.4:9091/health
min_connected_bootstrap: 2
bootstrap_retry_interval_seconds: 15
bootstrap_retry_max_backoff_seconds: 60
refuse_work_below_min_bootstrap: true
"#;
        fs::write(&path, body).expect("write temp bootstrap ring config");
        let result = BootstrapRing::from_config(path.as_path());
        fs::remove_file(&path).ok();
        assert!(result.is_err());
    }

    #[test]
    fn accepts_valid_unique_addr_threshold() {
        let path = temp_yaml_path("ok");
        let body = r#"
bootstrap_peers:
  - region: us-east-1
    label: "A"
    addr: /ip4/1.2.3.4/tcp/4001/p2p/peer-a
    health_url: http://1.2.3.4:9091/health
  - region: eu-west-1
    label: "B"
    addr: /ip4/5.6.7.8/tcp/4001/p2p/peer-b
    health_url: http://5.6.7.8:9091/health
min_connected_bootstrap: 2
bootstrap_retry_interval_seconds: 15
bootstrap_retry_max_backoff_seconds: 60
refuse_work_below_min_bootstrap: true
"#;
        fs::write(&path, body).expect("write temp bootstrap ring config");
        let result = BootstrapRing::from_config(path.as_path());
        fs::remove_file(&path).ok();
        assert!(result.is_ok());
    }
}
