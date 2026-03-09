> [!WARNING]
> **DEPRECATED**: This development plan is obsolete and represents early design iterations / bootstrapping instructions. Retained for historical context only.

Never scaffold. Never stub. Never leave a TODO. If a step says "implement X," X must be fully working code before you continue.

The full stack is:

desktop/rust/ — Rust daemon (libp2p + axum), the P2P nerve center
web/ — Next.js SPA for browser scouts (WebGPU)
sdk/python/ — Client lib
integrations/overflow — Standalone circuit breaker for verifier clusters

# Phase 1: Benchmark Control

Build a repeatable harness that proves whether Browser Scouts + Verifiers are actually faster than Verifiers alone.

## 1.1 — Synthetic Benchmark Program
This program does not need real WebGPU scouts. It simulates the P2P traffic pattern to baseline the daemon.

### 1.1.1 — Create benchmarks/run_all.py
"""
Shard Benchmark Harness — Entry Point
Usage: python benchmarks/run_all.py --scouts 100 --duration 300 --base-url http://localhost:9091
"""
import argparse
import asyncio
import json
import time
import statistics
from pathlib import Path

from benchmarks.scout_simulator import ScoutSimulator
from benchmarks.metrics_collector import MetricsCollector
from benchmarks.report_generator import ReportGenerator
from benchmarks.gates import evaluate_gates

async def main():
    parser = argparse.ArgumentParser(description="Shard Benchmark Harness")
    parser.add_argument("--scouts", type=int, default=100, help="Number of concurrent simulated Scout nodes")
    parser.add_argument("--duration", type=int, default=300, help="Benchmark duration in seconds")
    parser.add_argument("--base-url", type=str, default="http://localhost:9091", help="Target Verifier URL")
    parser.add_argument("--out-dir", type=str, default="benchmarks/results", help="Where to save results")
    args = parser.parse_args()

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    print(f"🚀 Starting Benchmark: {args.scouts} scouts, {args.duration}s...")
    
    # 1. Measure Verifier Baseline (No Scouts)
    baseline_latencies = await measure_baseline(args.base_url, samples=10)
    print(f"📊 Verifier-only p95: {statistics.quantiles(baseline_latencies, n=20)[18]:.2f}ms")

    # 2. Run Simulated Load
    simulator = ScoutSimulator(args.base_url, args.scouts, args.duration)
    collector = MetricsCollector()
    simulator.on_request_complete(collector.add_request)
    
    await simulator.run()

    results = collector.summarize(baseline_latencies=baseline_latencies)
    
    # Write JSON results
    results_path = out_dir / "latest.json"
    with open(results_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"Results written to {results_path}")

    # Write Markdown report
    report = ReportGenerator(results).render()
    report_path = out_dir / "report.md"
    with open(report_path, "w") as f:
        f.write(report)
    
    # Evaluate Gates
    passed, failures = evaluate_gates(results)
    if passed:
        print("\n✅ ALL GATES PASSED — Phase 1 benchmark complete")
    else:
        print("\n❌ GATE FAILURES:")
        for f in failures:
            print(f"  - {f}")
        raise SystemExit(1)

async def measure_baseline(url: str, samples: int) -> list[float]:
    import httpx
    latencies = []
    async with httpx.AsyncClient(timeout=30) as client:
        for _ in range(samples):
            start = time.monotonic()
            resp = await client.post(f"{url}/v1/chat/completions", json={
                "model": "shard-hybrid",
                "messages": [{"role": "user", "content": "hello"}],
                "max_tokens": 1,
                "stream": False
            })
            resp.raise_for_status()
            latencies.append((time.monotonic() - start) * 1000)
    return latencies

if __name__ == "__main__":
    asyncio.run(main())

### 1.1.2 — Create benchmarks/scout_simulator.py
Implement a class that spawns `N` tasks. Each task:
1. Polls `/v1/scout/work`.
2. Checks for new `work_id`.
3. If work found, waits 150ms (simulated generation).
4. Submits draft to `/v1/scout/draft`.
Record per-request metrics: draft_accepted: bool, latency_ms: float, tokens_in_draft: int, error: str | None.

### 1.1.3 — Create benchmarks/metrics_collector.py
This file must collect all per-request metrics from the simulator and compute:
- total_requests: int
- total_errors: int
- error_rate_pct: float
- acceptance_rate_pct: float — % of drafts accepted by verifier
- latency_p50_ms: float
- latency_p95_ms: float
- throughput_tps: float
- savings_pct: float — improvement over baseline

### 1.1.4 — Create benchmarks/gates.py
Define thresholds:
- `error_rate_pct < 0.1`
- `acceptance_rate_pct > 80.0`
- `savings_pct > 25.0`

### 1.1.5 — Create benchmarks/report_generator.py
Implement `ReportGenerator.render() -> str` that produces a Markdown report containing:
- A header with timestamp, scout count, and duration
- A results table with all metrics and their pass/fail status against gates
- A section listing all gate failures (or "ALL GATES PASSED")
- A raw JSON appendix of the full results dict

### 1.1.6 — Verification
```bash
# Start the daemon first
docker compose up --build shard-daemon -d

# Run the benchmark
python benchmarks/run_all.py --scouts 50 --duration 60
```

## 1.2 — WebGPU Hardware Telemetry
We need to know the eligibility of the current browser-pool hardware.

### 1.2.1 — Implement web/src/lib/hardware-probe.ts
Write a function `probeWebGPUDevice()` that returns:
- `can_speculate: bool`
- `vram_estimate_gb: number`
- `vendor: string`
- `device: string`

Implementation details:
- Call `navigator.gpu.requestAdapter()`
- Call `adapter.requestAdapterInfo()` to get vendor and device strings
- Call `adapter.requestDevice()` and check `device.limits.maxStorageBufferBindingSize` — must be >= 2GB
- Check that the adapter supports compute shaders by verifying `adapter.features.has('shader-f16')` or fallback float32
- Estimate VRAM by reading `adapter.limits.maxBufferSize` as a proxy (actual VRAM is not directly exposed)

### 1.2.2 — API Export
In `desktop/rust/src/api/telemetry.rs`, implement:
- `POST /v1/telemetry/webgpu` — Accept the JSON probe result, validate it, and store it in an in-memory aggregator (a `Mutex<WebGPUStats>` struct).
- `GET /metrics/webgpu-coverage` — Return a JSON summary:
```json
{
  "total_probes": 1247,
  "eligible_pct": 61.3,
  "high_performance_pct": 44.1,
  "low_power_pct": 17.2,
  "ineligible_pct": 38.7,
  "ineligible_reason_breakdown": {
    "no_navigator_gpu": 22.1,
    "no_adapter": 10.4,
    "insufficient_vram": 6.2
  }
}
```

## 1.3 — Formal Verification Protocol
What you are building: A formal specification of how draft tokens are accepted or rejected, plus a test suite that proves the logic is correct and adversarially robust.

### 1.3.1 — Write docs/verification-protocol.md
This document must contain the following sections, each fully written (not placeholder text):
- Section 1: Overview — One paragraph explaining speculative decoding and how Shard applies it.
- Section 2: The Acceptance Threshold — Define the scoring algorithm (likely KL-divergence between Scout's head and Verifier's head).
- Section 3: Result Encoding — Detail the `X-Shard-Acceptance` and `X-Shard-Draft-Id` headers.
- Section 4: The Golden Ticket Audit — Every Nth request (randomized), the Verifier ignores the draft and runs full inference.
  - The Scout's draft is compared against this independent reference
  - If the Scout's acceptance rate on golden tickets falls below Q% (define Q), the Scout's wallet is flagged
  - Flagged wallets receive no credits for 24 hours and are re-audited
- Section 5: Adversarial Attack Vectors and Defenses — Document at minimum: (a) plausible-but-wrong draft attack, (b) acceptance rate gaming, (c) timing attack to detect audit requests.

### 1.3.2 — Write tests/test_verification.py
Using `pytest` and `httpx`, write a suite that runs against a live local daemon:
```python
def test_draft_acceptance_score_logic():
    # Submit draft that matches verifier
    # Assert: tokens accepted, savings reported
    
def test_draft_rejection_on_mismatch():
    # Submit draft with wrong tokens
    # Assert: tokens rejected, verifier falls back to baseline

def test_golden_ticket_audit_triggered():
    # Mock random.random() to return a value below the audit probability
    # Assert: verifier runs independently, Scout draft is compared to reference
    # Assert: audit result is logged

def test_golden_ticket_wallet_flagging():
    # Simulate a Scout with consistently bad golden ticket scores
    # After Q failures, assert wallet is flagged and credits suspended

def test_acceptance_rate_below_threshold_triggers_flag():
    # Assert: daemon rejects scouts with historic low scores even if current draft is ok
```

# Phase 2: Mesh Formation (Gossipsub Implementation)

## 2.1 — Bootstrap Ring Config
The first 5 nodes in the network are fixed and mutually discovered.

### 2.1.1 — Create deploy/config/bootstrap-ring.yaml
```yaml
# Bootstrap Ring Configuration
# All 5 peers must be running before the ring is considered healthy.
# Replace <IP> and <PEER_ID> with actual values after provisioning.

bootstrap_peers:
  - region: us-east-1
    label: "US East (N. Virginia)"
    addr: /ip4/REPLACE_US_EAST_IP/tcp/4001/p2p/REPLACE_US_EAST_PEER_ID
    health_url: http://REPLACE_US_EAST_IP:9091/health

  - region: eu-west-1
    label: "Europe (Ireland)"
    addr: /ip4/REPLACE_EU_WEST_IP/tcp/4001/p2p/REPLACE_EU_WEST_PEER_ID
    health_url: http://REPLACE_EU_WEST_IP:9091/health

  - region: ap-southeast-1
    label: "Asia Pacific (Singapore)"
    addr: /ip4/REPLACE_AP_SOUTH_IP/tcp/4001/p2p/REPLACE_AP_SOUTH_PEER_ID
    health_url: http://REPLACE_AP_SOUTH_IP:9091/health
```

### 2.1.2 — Implement desktop/rust/src/network/bootstrap.rs
This module must fully implement:
- **`BootstrapRing` struct** with fields: `peers: Vec<BootstrapPeer>`, `min_connected: usize`, `connected: Arc<Mutex<HashSet<PeerId>>>`, `retry_handles: Vec<JoinHandle<()>>`
- **`BootstrapRing::load()`** — Reads the YAML and initializes PeerIds.
- **`BootstrapRing::maintain()`** — A background loop that:
  1. Checks current libp2p peers.
  2. If `connected.len() < min_connected`, identifies missing peers.
  3. Attempts `swarm.dial()` for missing peers.
  4. Polls `/health` of missing peers to verify responsiveness.
  5. Backs off exponentially on failure.

## 2.2 — Gossipsub Topics
Implement the following topics in `desktop/rust/src/network/swarm.rs`:

1.  **`shard-work`**
    - Payload: `{ "request_id": UUID, "prompt": string, "max_tokens": int, "timestamp": int }`
    - Logic: When verifier receives a client request, it broadcasts to this topic.
2.  **`shard-work-result`**
    - Payload: `{ "request_id": UUID, "draft_id": UUID, "accepted_tokens": int, "status": "ok" | "error" }`
    - Logic: When a verifier completes a request using a scout's draft, it broadcasts the result so the scout's credit can be settled (Phase 3).
3.  **`shard-node-health`**
    - Payload: `{ "peer_id": string, "load": float, "version": string, "model_status": string }`
    - Logic: Every 5 seconds, verifiers broadcast their status. Peers use this to build an in-memory `MeshTopology` for overflow routing.

# Phase 3: The Credit Economy (Proof of Contribution)

## 3.1 — Signature Envelopes
Every scout draft must be signed by the scout's Ed25519 identity.

### 3.1.1 — Implement desktop/rust/src/crypto/envelope.rs
```rust
struct SignedEnvelope<T> {
    payload: T,
    signer: PeerId,
    signature: Vec<u8>,
    nonce: u64,
}

impl<T: Serialize> SignedEnvelope<T> {
    fn verify(&self) -> bool { ... }
}
```

## 3.2 — The Ledger (In-Memory/SQLite)
For Phase 3, we don't need a blockchain. We need a reliable local ledger that verifiers sync.

### 3.2.1 — Implement desktop/rust/src/ledger/manager.rs
- `CreditLedger` struct backed by SQLite.
- **`settle_contribution(request_id, scout_id, tokens)`**:
  1. Verify the `shard-work-result` gossip message.
2. Ensure `request_id` has not been settled yet (idempotency).
  3. Update `scout_id` balance in SQLite: `balance += tokens`.
  4. Broadcast `ledger-update` to the mesh.

### 3.2.2 — Ledger Sync
- Topic: `shard-ledger-sync`
- Pattern: Request-Response. New nodes use libp2p `request-response` protocol to ask bootstrap peers for the latest SQLite db snapshot (compressed).

## 3.3 — The Economic API
In `desktop/rust/src/api/ledger.rs`, implement:
- `GET /v1/scout/balance` — Returns the credit balance for the provided PeerId.
- `GET /v1/scout/history` — Returns the last 50 contribution events.

# Phase 4: Production Hardening

## 4.1 — Deployment Orchestration

### 4.1.1 — Update Docker Compose
Update the root `docker-compose.yml` to include:
- `shard-daemon`
- `prometheus` (scraping Shard `/metrics`)
- `grafana` (pre-configured with a dashboard showing THROUGHPUT vs LATENCY)

### 4.1.2 — Create scripts/stress_test.sh
A bash script that:
1. Scales `shard-daemon` to 3 replicas.
2. Runs `benchmarks/run_all.py` with 1000 scouts.
3. Kills one daemon replica mid-run.
4. Asserts that the benchmark continues with < 1% error rate (P2P failover check).

## 4.2 — Documentation
Finalize the following:
- `docs/run-a-node.md` — Step-by-step for Ubuntu 22.04 with a 3060/4090.
- `docs/api.md` — Full OpenAPI spec (use Swagger UI in dev).
- `docs/credit-economy.md` — Explain the math behind token-to-credit conversion.
- `docs/verification-protocol.md` — (Ensure 1.3.1 is actually thorough).

## 4.3 — Final Signoff
Run `benchmarks/run_all.py` one last time. Capture the `report.md`. This is the evidence for the Shard Beta Launch.
```bash
python benchmarks/run_all.py --scouts 250 --duration 3600 --out-dir reports/beta-launch
```
If `savings_pct > 40%`, Shard is ready for the public Internet.
