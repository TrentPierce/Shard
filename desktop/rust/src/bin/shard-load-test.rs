use clap::Parser;
use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;

#[derive(Parser, Debug)]
#[command(
    name = "shard-load-test",
    version,
    about = "Shard synthetic load test tool"
)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:9091")]
    base_url: String,
    #[arg(long, default_value = "100")]
    requests: usize,
    #[arg(long, default_value = "100")]
    concurrency: usize,
    #[arg(long, default_value = "all")]
    mode: String,
    #[arg(long, default_value = "benchmarks")]
    out_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkRequest {
    request_id: String,
    prompt_context: String,
    min_tokens: i32,
    created_at_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftResultSubmission {
    work_id: String,
    scout_id: String,
    draft_text: String,
    timestamp: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedEnvelope<T> {
    signer_pubkey_hex: String,
    nonce: u64,
    timestamp_ms: u128,
    payload_hash_hex: String,
    payload: T,
    signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedRequest<T> {
    envelope: SignedEnvelope<T>,
}

#[derive(Debug, Clone, Serialize)]
struct ModeReport {
    mode: String,
    requests: usize,
    concurrency: usize,
    successes: usize,
    failures: usize,
    throughput_rps: f64,
    average_latency_ms: f64,
    p50_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    offload_percent: f64,
    failure_rate_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
struct BenchmarkReport {
    timestamp_ms: u128,
    baseline: Option<ModeReport>,
    distributed_signed: Option<ModeReport>,
    signature_validation_overhead_ms: f64,
    estimated_gpu_savings_percent: f64,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

fn sign_payload<T: Serialize + Clone>(
    payload: T,
    key: &SigningKey,
    nonce: u64,
    timestamp_ms: u128,
) -> SignedRequest<T> {
    let signer_pubkey_hex = hex::encode(key.verifying_key().to_bytes());
    let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
    let payload_hash_hex = hex::encode(Sha256::digest(payload_bytes));
    let body = format!(
        "{}|{}|{}|{}",
        signer_pubkey_hex, nonce, timestamp_ms, payload_hash_hex
    );
    let signature_hex = hex::encode(key.sign(body.as_bytes()).to_bytes());
    SignedRequest {
        envelope: SignedEnvelope {
            signer_pubkey_hex,
            nonce,
            timestamp_ms,
            payload_hash_hex,
            payload,
            signature_hex,
        },
    }
}

async fn run_mode(
    client: &Client,
    args: &Args,
    mode: &str,
    key: &SigningKey,
) -> anyhow::Result<ModeReport> {
    let mode_owned = mode.to_string();
    let sem = Arc::new(Semaphore::new(args.concurrency));
    let started = Instant::now();
    let mut tasks = Vec::with_capacity(args.requests);
    let signed = mode == "distributed";

    for i in 0..args.requests {
        let permit = sem.clone().acquire_owned().await?;
        let client = client.clone();
        let base_url = args.base_url.clone();
        let key = key.clone();
        let mode = mode_owned.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            let req_id = format!("{}-{}", mode, i);
            let start = Instant::now();

            let work = WorkRequest {
                request_id: req_id.clone(),
                prompt_context: "benchmark prompt".to_string(),
                min_tokens: 2,
                created_at_ms: Some(now_ms()),
            };

            let sent = if signed {
                let body = sign_payload(work, &key, (i as u64) * 2 + 1, now_ms());
                client
                    .post(format!("{base_url}/signed/broadcast-work"))
                    .json(&body)
                    .send()
                    .await
            } else {
                client
                    .post(format!("{base_url}/broadcast-work"))
                    .json(&work)
                    .send()
                    .await
            };

            if sent.is_err() {
                return (false, start.elapsed().as_millis() as f64, 0usize);
            }

            let draft = DraftResultSubmission {
                work_id: req_id.clone(),
                scout_id: hex::encode(key.verifying_key().to_bytes()),
                draft_text: "token-a token-b token-c".to_string(),
                timestamp: Some((now_ms() as f64) / 1000.0),
            };

            let submit_ok = if signed {
                let body = sign_payload(draft, &key, (i as u64) * 2 + 2, now_ms());
                client
                    .post(format!("{base_url}/signed/submit-draft"))
                    .json(&body)
                    .send()
                    .await
            } else {
                client
                    .post(format!("{base_url}/submit-draft"))
                    .json(&draft)
                    .send()
                    .await
            };

            if submit_ok.is_err() {
                return (false, start.elapsed().as_millis() as f64, 0usize);
            }

            for _ in 0..20 {
                let polled = client
                    .get(format!("{base_url}/pop-result"))
                    .query(&[("request_id", req_id.as_str())])
                    .send()
                    .await;
                if let Ok(res) = polled {
                    if let Ok(v) = res.json::<serde_json::Value>().await {
                        if v.get("result").is_some() && !v["result"].is_null() {
                            return (true, start.elapsed().as_millis() as f64, 3usize);
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }

            (false, start.elapsed().as_millis() as f64, 0usize)
        }));
    }

    let mut latencies = Vec::with_capacity(args.requests);
    let mut successes = 0usize;
    let mut failures = 0usize;
    let mut offloaded_tokens = 0usize;
    for task in tasks {
        let (ok, latency, tokens) = task.await?;
        latencies.push(latency);
        if ok {
            successes += 1;
            offloaded_tokens += tokens;
        } else {
            failures += 1;
        }
    }

    let elapsed_s = started.elapsed().as_secs_f64().max(0.001);
    let throughput = successes as f64 / elapsed_s;
    let average = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<f64>() / latencies.len() as f64
    };

    Ok(ModeReport {
        mode: mode.to_string(),
        requests: args.requests,
        concurrency: args.concurrency,
        successes,
        failures,
        throughput_rps: throughput,
        average_latency_ms: average,
        p50_latency_ms: percentile(&latencies, 0.50),
        p95_latency_ms: percentile(&latencies, 0.95),
        p99_latency_ms: percentile(&latencies, 0.99),
        offload_percent: if successes == 0 {
            0.0
        } else {
            (offloaded_tokens as f64 / (successes as f64 * 3.0)) * 100.0
        },
        failure_rate_percent: if args.requests == 0 {
            0.0
        } else {
            (failures as f64 / args.requests as f64) * 100.0
        },
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let key = SigningKey::from_bytes(&[9u8; 32]);
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    let baseline = if args.mode == "all" || args.mode == "baseline" {
        Some(run_mode(&client, &args, "baseline", &key).await?)
    } else {
        None
    };
    let distributed_signed = if args.mode == "all" || args.mode == "distributed" {
        Some(run_mode(&client, &args, "distributed", &key).await?)
    } else {
        None
    };

    let signature_overhead = match (&baseline, &distributed_signed) {
        (Some(b), Some(d)) => (d.average_latency_ms - b.average_latency_ms).max(0.0),
        _ => 0.0,
    };
    let savings = distributed_signed
        .as_ref()
        .map(|d| (d.offload_percent * 0.8).min(95.0))
        .unwrap_or(0.0);

    let report = BenchmarkReport {
        timestamp_ms: now_ms(),
        baseline,
        distributed_signed,
        signature_validation_overhead_ms: signature_overhead,
        estimated_gpu_savings_percent: savings,
    };

    std::fs::create_dir_all(&args.out_dir)?;
    let stamp = report.timestamp_ms;
    let json_path = args.out_dir.join(format!("benchmark-{stamp}.json"));
    let md_path = args.out_dir.join(format!("benchmark-{stamp}.md"));
    std::fs::write(&json_path, serde_json::to_vec_pretty(&report)?)?;

    let md = format!(
        "# Shard Benchmark Report\n\n- Timestamp: `{}`\n- Signature validation overhead (ms): `{:.2}`\n- Estimated GPU savings (%): `{:.2}`\n\n```json\n{}\n```\n",
        report.timestamp_ms,
        report.signature_validation_overhead_ms,
        report.estimated_gpu_savings_percent,
        serde_json::to_string_pretty(&report)?
    );
    std::fs::write(&md_path, md)?;

    println!("{}", serde_json::to_string_pretty(&report)?);
    println!("wrote {}", json_path.display());
    println!("wrote {}", md_path.display());
    Ok(())
}
