use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use tokio_postgres::NoTls;

#[derive(Debug, Clone)]
pub struct PersistedNodeMetricReport {
    pub node_pubkey: String,
    pub role: String,
    pub queue_depth: u64,
    pub node_latency_ms: u64,
    pub uptime_seconds: u64,
    pub timestamp_ms: u128,
}

#[derive(Debug, Clone)]
pub enum MetricsPersistence {
    None,
    Sqlite { path: PathBuf },
    Postgres { dsn: String },
}

impl MetricsPersistence {
    pub async fn initialize(&self) -> Result<()> {
        match self {
            MetricsPersistence::None => Ok(()),
            MetricsPersistence::Sqlite { path } => {
                let path = path.clone();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    let conn = Connection::open(path)?;
                    conn.execute_batch(
                        "CREATE TABLE IF NOT EXISTS node_metrics_reports (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            node_pubkey TEXT NOT NULL,
                            role TEXT NOT NULL,
                            queue_depth INTEGER NOT NULL,
                            node_latency_ms INTEGER NOT NULL,
                            uptime_seconds INTEGER NOT NULL,
                            timestamp_ms TEXT NOT NULL
                        );
                        CREATE INDEX IF NOT EXISTS idx_node_metrics_reports_time ON node_metrics_reports(timestamp_ms);",
                    )?;
                    Ok(())
                })
                .await??;
                Ok(())
            }
            MetricsPersistence::Postgres { dsn } => {
                let (client, connection) = tokio_postgres::connect(dsn, NoTls).await?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::warn!(%e, "metrics postgres connection closed");
                    }
                });
                client
                    .batch_execute(
                        "CREATE TABLE IF NOT EXISTS node_metrics_reports (
                            id BIGSERIAL PRIMARY KEY,
                            node_pubkey TEXT NOT NULL,
                            role TEXT NOT NULL,
                            queue_depth BIGINT NOT NULL,
                            node_latency_ms BIGINT NOT NULL,
                            uptime_seconds BIGINT NOT NULL,
                            timestamp_ms NUMERIC NOT NULL
                        );
                        CREATE INDEX IF NOT EXISTS idx_node_metrics_reports_time ON node_metrics_reports(timestamp_ms);",
                    )
                    .await?;
                Ok(())
            }
        }
    }

    pub async fn persist_report(&self, report: &PersistedNodeMetricReport) -> Result<()> {
        match self {
            MetricsPersistence::None => Ok(()),
            MetricsPersistence::Sqlite { path } => {
                let path = path.clone();
                let report = report.clone();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    let conn = Connection::open(path)?;
                    conn.execute(
                        "INSERT INTO node_metrics_reports
                        (node_pubkey, role, queue_depth, node_latency_ms, uptime_seconds, timestamp_ms)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            report.node_pubkey,
                            report.role,
                            report.queue_depth as i64,
                            report.node_latency_ms as i64,
                            report.uptime_seconds as i64,
                            report.timestamp_ms.to_string(),
                        ],
                    )?;
                    Ok(())
                })
                .await??;
                Ok(())
            }
            MetricsPersistence::Postgres { dsn } => {
                let (client, connection) = tokio_postgres::connect(dsn, NoTls).await?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::warn!(%e, "metrics postgres connection closed");
                    }
                });
                client
                    .execute(
                        "INSERT INTO node_metrics_reports
                        (node_pubkey, role, queue_depth, node_latency_ms, uptime_seconds, timestamp_ms)
                        VALUES ($1, $2, $3, $4, $5, $6)",
                        &[
                            &report.node_pubkey,
                            &report.role,
                            &(report.queue_depth as i64),
                            &(report.node_latency_ms as i64),
                            &(report.uptime_seconds as i64),
                            &report.timestamp_ms.to_string(),
                        ],
                    )
                    .await?;
                Ok(())
            }
        }
    }
}
