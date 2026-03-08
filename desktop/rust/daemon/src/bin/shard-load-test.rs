use clap::Parser;
use ed25519_dalek::SigningKey;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use shard_common::common::signed_envelope::SignedEnvelope;
use shard_common::types::WorkRequest;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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
    #[arg(long = "header")]
    headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DraftResultSubmission {
    work_id: String,
    scout_id: String,
    #[serde(default)]
    lease_id: Option<String>,
    draft_text: String,
    #[serde(default)]
    prompt_context: Option<String>,
    #[serde(default)]
    draft_tokens: Vec<i32>,
    #[serde(default)]
    timestamp: Option<f64>,
    #[serde(default)]
    scout_mode: Option<String>,
    #[serde(default)]
    spot_check: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WorkPollResponse {
    work: Option<WorkRequest>,
    #[serde(default)]
    transient_error: bool,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    retry_after_ms: Option<u64>,
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

fn parse_headers(raw_headers: &[String]) -> anyhow::Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for raw in raw_headers {
        let Some((name, value)) = raw.split_once('=') else {
            anyhow::bail!("invalid --header value `{raw}`; expected NAME=VALUE");
        };
        let header_name = HeaderName::from_bytes(name.trim().as_bytes())?;
        let header_value = HeaderValue::from_str(value.trim())?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
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

fn make_signing_key(seed: u64) -> SigningKey {
    let mut sk_bytes = [0u8; 32];
    for b in 0..8 {
        sk_bytes[b] = (seed >> (b * 8)) as u8;
        sk_bytes[b + 8] = ((now_ms() as u64) >> (b * 8)) as u8;
    }
    SigningKey::from_bytes(&sk_bytes)
}

fn build_work_request(request_id: String) -> WorkRequest {
    WorkRequest {
        request_id,
        prompt_context: "benchmark prompt".to_string(),
        min_tokens: 2,
        created_at_ms: Some(now_ms()),
        lease_id: None,
        lease_expires_at_ms: None,
        assigned_scout_id: None,
        preferred_endpoint: None,
    }
}

async fn post_signed<T: Serialize + Clone>(
    client: &Client,
    url: String,
    payload: T,
    key: &SigningKey,
    nonce_counter: &Arc<AtomicU64>,
) -> reqwest::Result<reqwest::Response> {
    let nonce = nonce_counter.fetch_add(1, Ordering::Relaxed);
    let body = SignedRequest {
        envelope: SignedEnvelope::sign(payload, key, nonce, now_ms()),
    };
    client.post(url).json(&body).send().await
}

async fn ensure_pow_verified(
    client: &Client,
    base_url: &str,
    scout_pubkey: &str,
) -> anyhow::Result<()> {
    let response = client
        .get(format!(
            "{base_url}/v1/pow/challenge?peer_id={scout_pubkey}&hardware_concurrency=8"
        ))
        .send()
        .await?;
    let value = response.json::<serde_json::Value>().await?;
    let challenge = value
        .get("challenge")
        .ok_or_else(|| anyhow::anyhow!("missing pow challenge payload"))?;
    let challenge_hex = challenge["challenge_bytes_hex"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing pow challenge bytes"))?;
    let difficulty = challenge["difficulty"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("missing pow difficulty"))? as u8;
    let challenge_bytes = hex::decode(challenge_hex)?;
    let solution = tokio::task::spawn_blocking(move || {
        shard_common::common::pow_challenge::solve_challenge(&challenge_bytes, difficulty)
    })
    .await
    .map_err(|err| anyhow::anyhow!("pow worker join failed: {err}"))?
    .ok_or_else(|| anyhow::anyhow!("failed to solve pow challenge"))?;
    let verify = client
        .post(format!("{base_url}/v1/pow/verify"))
        .json(&serde_json::json!({
            "peer_id": scout_pubkey,
            "nonce": solution.nonce,
            "hash_hex": solution.hash_hex,
        }))
        .send()
        .await?;
    let verify_json = verify.json::<serde_json::Value>().await?;
    anyhow::ensure!(
        verify_json["ok"].as_bool() == Some(true),
        "pow verify rejected: {}",
        verify_json
    );
    Ok(())
}

async fn poll_result_until_complete(
    client: &Client,
    base_url: &str,
    request_id: &str,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let polled = client
            .get(format!("{base_url}/pop-result"))
            .query(&[("request_id", request_id)])
            .send()
            .await;
        if let Ok(res) = polled {
            if let Ok(v) = res.json::<serde_json::Value>().await {
                if v.get("result").is_some() && !v["result"].is_null() {
                    return true;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

async fn poll_for_work(
    client: &Client,
    base_url: &str,
    scout_id: &str,
) -> anyhow::Result<WorkPollResponse> {
    let response = client
        .get(format!("{base_url}/v1/scout/work?scout_id={scout_id}"))
        .send()
        .await?;
    let status = response.status();
    let payload = response
        .json::<WorkPollResponse>()
        .await
        .unwrap_or_else(|_| WorkPollResponse::default());
    if !status.is_success() {
        return Ok(WorkPollResponse {
            transient_error: status.as_u16() == 429 || status.as_u16() == 503 || status.is_server_error(),
            detail: payload
                .detail
                .or_else(|| Some(format!("HTTP {}", status.as_u16()))),
            retry_after_ms: payload.retry_after_ms,
            ..payload
        });
    }
    Ok(payload)
}

async fn submit_signed_draft(
    client: &Client,
    base_url: &str,
    key: &SigningKey,
    nonce_counter: &Arc<AtomicU64>,
    work: &WorkRequest,
    scout_id: &str,
) -> anyhow::Result<bool> {
    let draft = DraftResultSubmission {
        work_id: work.request_id.clone(),
        scout_id: scout_id.to_string(),
        lease_id: work.lease_id.clone(),
        draft_text: "token-a token-b token-c".to_string(),
        prompt_context: Some(work.prompt_context.clone()),
        draft_tokens: vec![128001, 128002, 128003],
        timestamp: Some((now_ms() as f64) / 1000.0),
        scout_mode: Some("synthetic".to_string()),
        spot_check: None,
    };
    let response = post_signed(
        client,
        format!("{base_url}/signed/submit-draft"),
        draft,
        key,
        nonce_counter,
    )
    .await?;
    let payload = response.json::<serde_json::Value>().await.unwrap_or_default();
    Ok(payload["ok"].as_bool() == Some(true))
}

fn mode_report(
    mode: &str,
    requests: usize,
    concurrency: usize,
    started: Instant,
    latencies: Vec<f64>,
    successes: usize,
    failures: usize,
    offloaded_tokens: usize,
) -> ModeReport {
    let elapsed_s = started.elapsed().as_secs_f64().max(0.001);
    let throughput = successes as f64 / elapsed_s;
    let average = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<f64>() / latencies.len() as f64
    };

    ModeReport {
        mode: mode.to_string(),
        requests,
        concurrency,
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
        failure_rate_percent: if requests == 0 {
            0.0
        } else {
            (failures as f64 / requests as f64) * 100.0
        },
    }
}

async fn run_baseline_mode(client: &Client, args: &Args) -> anyhow::Result<ModeReport> {
    let sem = Arc::new(Semaphore::new(args.concurrency));
    let started = Instant::now();
    let mut tasks = Vec::with_capacity(args.requests);

    for i in 0..args.requests {
        let permit = sem.clone().acquire_owned().await?;
        let client = client.clone();
        let base_url = args.base_url.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            let req_id = format!("baseline-{i}");
            let start = Instant::now();
            let work = build_work_request(req_id.clone());
            let sent = client
                .post(format!("{base_url}/broadcast-work"))
                .json(&work)
                .send()
                .await;
            if sent.is_err() {
                return (false, start.elapsed().as_millis() as f64, 0usize);
            }
            let ok =
                poll_result_until_complete(&client, &base_url, &req_id, Duration::from_secs(10))
                    .await;
            (ok, start.elapsed().as_millis() as f64, 0usize)
        }));
    }

    let mut latencies = Vec::with_capacity(args.requests);
    let mut successes = 0usize;
    let mut failures = 0usize;
    for task in tasks {
        let (ok, latency, _tokens) = task.await?;
        latencies.push(latency);
        if ok {
            successes += 1;
        } else {
            failures += 1;
        }
    }

    Ok(mode_report(
        "baseline",
        args.requests,
        args.concurrency,
        started,
        latencies,
        successes,
        failures,
        0,
    ))
}

async fn run_distributed_mode(client: &Client, args: &Args) -> anyhow::Result<ModeReport> {
    let started = Instant::now();
    let total_requests = args.requests;
    let concurrency = args.concurrency;
    let nonce_counter = Arc::new(AtomicU64::new((now_ms() % 1_000_000_000) as u64 * 1000));
    let finished = Arc::new(AtomicUsize::new(0));
    let offloaded_tokens = Arc::new(AtomicUsize::new(0));

    let scout_workers = concurrency.clamp(1, total_requests.max(1));
    let mut scouts = Vec::with_capacity(scout_workers);
    for scout_index in 0..scout_workers {
        let client = client.clone();
        let base_url = args.base_url.clone();
        let nonce_counter = nonce_counter.clone();
        let finished = finished.clone();
        let offloaded_tokens = offloaded_tokens.clone();
        scouts.push(tokio::spawn(async move {
            let key = make_signing_key(10_000 + scout_index as u64);
            let scout_id = hex::encode(key.verifying_key().to_bytes());
            if ensure_pow_verified(&client, &base_url, &scout_id).await.is_err() {
                return;
            }
            let mut idle_polls = 0usize;
            loop {
                if finished.load(Ordering::Relaxed) >= total_requests && idle_polls >= 8 {
                    break;
                }
                match poll_for_work(&client, &base_url, &scout_id).await {
                    Ok(payload) => {
                        if let Some(work) = payload.work {
                            idle_polls = 0;
                            if submit_signed_draft(
                                &client,
                                &base_url,
                                &key,
                                &nonce_counter,
                                &work,
                                &scout_id,
                            )
                            .await
                            .unwrap_or(false)
                            {
                                offloaded_tokens.fetch_add(3, Ordering::Relaxed);
                            }
                        } else {
                            idle_polls = idle_polls.saturating_add(1);
                            let delay_ms = payload.retry_after_ms.unwrap_or(40).clamp(10, 500);
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        }
                    }
                    Err(_) => {
                        idle_polls = idle_polls.saturating_add(1);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        }));
    }

    let sem = Arc::new(Semaphore::new(concurrency));
    let mut tasks = Vec::with_capacity(total_requests);
    for i in 0..total_requests {
        let permit = sem.clone().acquire_owned().await?;
        let client = client.clone();
        let base_url = args.base_url.clone();
        let nonce_counter = nonce_counter.clone();
        let finished = finished.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            let req_id = format!("distributed-{i}");
            let start = Instant::now();
            let key = make_signing_key(i as u64);
            let work = build_work_request(req_id.clone());
            let sent = post_signed(
                &client,
                format!("{base_url}/signed/broadcast-work"),
                work,
                &key,
                &nonce_counter,
            )
            .await;
            if sent.is_err() {
                finished.fetch_add(1, Ordering::Relaxed);
                return (false, start.elapsed().as_millis() as f64);
            }
            let ok =
                poll_result_until_complete(&client, &base_url, &req_id, Duration::from_secs(12))
                    .await;
            finished.fetch_add(1, Ordering::Relaxed);
            (ok, start.elapsed().as_millis() as f64)
        }));
    }

    let mut latencies = Vec::with_capacity(args.requests);
    let mut successes = 0usize;
    let mut failures = 0usize;
    for task in tasks {
        let (ok, latency) = task.await?;
        latencies.push(latency);
        if ok {
            successes += 1;
        } else {
            failures += 1;
        }
    }
    for scout in scouts {
        let _ = scout.await;
    }

    Ok(mode_report(
        "distributed",
        total_requests,
        concurrency,
        started,
        latencies,
        successes,
        failures,
        offloaded_tokens.load(Ordering::Relaxed),
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let headers = parse_headers(&args.headers)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .default_headers(headers)
        .build()?;

    let baseline = if args.mode == "all" || args.mode == "baseline" {
        Some(run_baseline_mode(&client, &args).await?)
    } else {
        None
    };
    let distributed_signed = if args.mode == "all" || args.mode == "distributed" {
        Some(run_distributed_mode(&client, &args).await?)
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

    let mut md = format!(
        "# Shard Benchmark Report\n\n- **Timestamp:** `{}`\n- **Signature validation overhead:** `{:.2} ms`\n- **Estimated GPU savings:** `{:.2}%`\n\n",
        report.timestamp_ms,
        report.signature_validation_overhead_ms,
        report.estimated_gpu_savings_percent
    );

    md.push_str("## Mode Results\n\n");
    md.push_str("| Mode | Requests | Concurrency | RPS | Success | Failure Rate | Avg Latency | P50 | P95 | P99 | Offload |\n");
    md.push_str("|---|---|---|---|---|---|---|---|---|---|---|\n");

    if let Some(ref b) = report.baseline {
        md.push_str(&format!(
            "| {} | {} | {} | {:.2} | {}/{} | {:.2}% | {:.2}ms | {:.2}ms | {:.2}ms | {:.2}ms | {:.2}% |\n",
            b.mode, b.requests, b.concurrency, b.throughput_rps,
            b.successes, b.requests, b.failure_rate_percent,
            b.average_latency_ms, b.p50_latency_ms, b.p95_latency_ms, b.p99_latency_ms,
            b.offload_percent
        ));
    }
    if let Some(ref d) = report.distributed_signed {
        md.push_str(&format!(
            "| {} | {} | {} | {:.2} | {}/{} | {:.2}% | {:.2}ms | {:.2}ms | {:.2}ms | {:.2}ms | {:.2}% |\n",
            d.mode, d.requests, d.concurrency, d.throughput_rps,
            d.successes, d.requests, d.failure_rate_percent,
            d.average_latency_ms, d.p50_latency_ms, d.p95_latency_ms, d.p99_latency_ms,
            d.offload_percent
        ));
    }

    md.push_str("\n<details><summary>Raw JSON Dataset</summary>\n\n");
    md.push_str("```json\n");
    md.push_str(&serde_json::to_string_pretty(&report)?);
    md.push_str("\n```\n</details>\n");

    std::fs::write(&md_path, md)?;

    println!("{}", serde_json::to_string_pretty(&report)?);
    println!("wrote {}", json_path.display());
    println!("wrote {}", md_path.display());
    Ok(())
}
