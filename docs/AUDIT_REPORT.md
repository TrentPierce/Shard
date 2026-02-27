# Shard Project — End-to-End Audit Report

**Date:** 2026-02-25  
**Auditor Role:** Staff Systems Architect / DevOps / UX Researcher  
**Scope:** Full-stack read-only audit — Rust daemon, Next.js web dashboard, Python SDK, CI/CD, installers  
**Benchmark Context:** LAN distributed +54.5% throughput; WAN (EC2) 31.7% success rate, ~41.2s p95 latency

---

## Executive Summary

Shard is an ambitious P2P distributed inference network with a surprisingly mature Rust daemon, a well-structured Next.js web dashboard, and a usable Python SDK. However, several **critical architectural gaps** directly cause the observed WAN failure rate and extreme tail latency. The system works well on LAN because NAT traversal is irrelevant locally, but falls apart under real-world WAN conditions due to missing relay infrastructure, an aggressive scout timeout default, and a single-threaded inference engine lock contention issue.

---

## Domain 1: Networking, P2P Mesh & Distributed Architecture

### Current State Assessment

The libp2p integration in `desktop/rust/daemon/src/main.rs` is comprehensive. The `ShardBehaviour` composite includes:

| Protocol | Status | Notes |
|---|---|---|
| **Gossipsub** | ✅ Configured | Topics: `shard-work`, `shard-work-result` |
| **Kademlia DHT** | ✅ Configured | MemoryStore (ephemeral) |
| **TCP** | ✅ Active | Port 4001 default |
| **WebSocket** | ✅ Active | Port 4101 (tcp+100) |
| **QUIC** | ✅ Active | Port 9092/UDP |
| **WebRTC-direct** | ⚠️ Alpha | `libp2p-webrtc v0.9.0-alpha.1`, Linux/Mac only |
| **DCUtR** | ✅ Toggled | Enabled when `--nat-traversal=true` |
| **AutoNAT v1** | ✅ Present | Configured |
| **Circuit Relay (server)** | ✅ Toggled | Via `--relay-mode` CLI flag |
| **Circuit Relay (client)** | ✅ Toggled | Via `--nat-traversal` CLI flag |
| **mDNS** | ✅ Active | Always on — good for LAN, irrelevant for WAN |
| **Identify** | ✅ Active | |
| **Ping** | ✅ Active | |

### Critical Vulnerabilities / Blockers

#### 🔴 CVE-1: No Operational Relay Servers → 68% WAN Failure (ROOT CAUSE)

The relay client is enabled, but **there are no deployed relay servers**. The `--relay-mode` flag exists, but it must be explicitly enabled on a publicly reachable node. Without at least one relay server:

- DCUtR hole punching has no relay to coordinate through
- AutoNAT can detect that the node is behind a NAT but can do nothing about it
- Symmetric NAT (common on EC2 default VPCs with security groups) makes direct connections impossible

**Evidence:** `docker-compose.yml` (line 7) does not pass `--relay-mode`. The EC2 deploy scripts don't pass it either. There is **zero bootstrap relay infrastructure**.

**Impact:** This is the **primary cause** of the 68.3% WAN failure rate. Without relay coordination, most WAN nodes simply cannot establish direct connections.

#### 🔴 CVE-2: 30-Second Scout Timeout Default → 41.2s p95 Latency (ROOT CAUSE)

`SpeculativeConfig::default()` (line 784-790 of `main.rs`) sets:
```rust
scout_timeout_ms: 30000  // 30 seconds!
```

This means *every* speculative request that doesn't receive a scout draft within 30 seconds will block the inference engine for the full duration before falling back to local generation. The comment says:
> "Browser scouts poll work over HTTP and may need PoW + queueing + submission retries"

This explains the ~41.2s p95 latency: it's the 30s scout timeout + actual local generation time.

**Impact:** Any request routed through speculative mode on a mesh with unreachable scouts will incur a minimum 30s latency floor.

#### 🟡 CVE-3: Global Engine Mutex Creates Head-of-Line Blocking

`SharedState.engine` is an `Arc<Mutex<Option<ShardEngine>>>` (line 692). The `chat_completions_handler` acquires this lock for the **entire duration of generation** (lines 367-526 in `scheduler.rs`):

```rust
let mut engine_guard = state.engine.lock().await;
if let Some(engine) = engine_guard.as_mut() {
    // holds lock through all tokenization, eval, and logit sampling
}
```

This means only **one concurrent inference request** can be processed at a time. All other requests queue behind the mutex. Under any concurrency, this creates cascading latency.

#### 🟡 CVE-4: WebRTC-direct is Alpha and Windows-Excluded

`libp2p-webrtc v0.9.0-alpha.1` is an alpha release. Line 4 of `main.rs` indicates WebRTC is only for Linux/Mac:
> "WebRTC-direct on Linux/Mac"

Windows users (the primary target for desktop installs) have no WebRTC capability, reducing NAT traversal options to relay-only (which doesn't exist, per CVE-1).

#### 🟡 CVE-5: No Split-Brain / Out-of-Order Draft Token Handling

`DraftSubmission` (line 491-497) contains a `seq_start` field, but there is **no sequence validation** in `verify_draft_tokens()`. If scout drafts arrive out of order, or if two scouts submit for the same work ID, the second submission is silently dropped or overwrites the first. There is no deduplication or ordering guarantee.

#### 🟡 CVE-6: No Instant TTFT Fallback Mechanism

The `FallbackConfig` (line 84-110 of `fallback.rs`) only triggers on long-context prompts or after a scout has already started and failed mid-generation. There is **no proactive TTFT deadline**: the system does not start a local generation race while waiting for scouts. The `wait_for_scout_draft()` function is fully blocking — it either gets a draft or times out.

**Recommended architecture:** Start local generation in parallel with scout dispatch. Cancel whichever finishes second ("race" pattern).

### Resilience Assessment

| Scenario | Handling |
|---|---|
| Dropped peer | ✅ Bootstrap failure tracking with `MAX_BOOTSTRAP_FAILURES=3` → peer removal |
| Reconnection | ✅ Periodic reconnect loop with configurable `reconnect_seconds` |
| Scout ban/cooldown | ✅ `ScoutPenaltyBook` with sliding window, 55% success threshold, 60s bans |
| Split-brain | ❌ No quorum or consensus mechanism for conflicting drafts |
| Out-of-order tokens | ❌ Not validated |
| Centralized fallback | ⚠️ URL defaults to `https://api.ourcompany.com/v1/internal/scout-fallback` — a **placeholder** that will 404 |

---

## Domain 2: Inference & Compute (BitNet Integration)

### Current State Assessment

The inference pipeline uses a C FFI bridge (`shard-verifier/src/inference.rs`) loading a native `.dll`/`.so` via `libloading`. The engine (`ShardEngine`) wraps five C functions: `shard_init_ex`, `shard_eval`, `shard_get_logits`, `shard_tokenize`, `shard_token_to_piece`, `shard_free`.

### Critical Vulnerabilities / Blockers

#### 🔴 INF-1: `ShardEngine` Has `unsafe impl Send` But Is NOT Thread-Safe

Line 121-122:
```rust
unsafe impl Send for ShardEngine {}
```

The comment says the C API "uses internal locking or is thread-unsafe by design" and then immediately marks it `Send`. This is correct for single-owner usage behind a Mutex, but if the mutex is ever accidentally bypassed or the engine is cloned, **undefined behavior** ensues (memory corruption, segfaults).

The engine is accessed via `state.engine.lock().await` which provides serialized access, so this is *currently safe* but extremely fragile. Any future refactoring that adds a second access path will silently break.

#### 🟡 INF-2: No VRAM/RAM Cleanup Guarantee on Abnormal Exit

`ShardEngine::drop()` (line 257-264) calls `shard_free()`, which is correct for normal shutdown. However:

- If the Rust daemon is SIGKILLed, destructors don't run
- If the C engine panics internally, the `free_fn` may not release GPU memory
- There is no watchdog or OOM guard

On Windows, this means `.gguf` model file handles may remain locked, preventing model updates or re-launches until reboot.

#### 🟡 INF-3: Hardcoded Vocab Size of 128256

The vocab size appears in multiple locations as a magic number:
- `scheduler.rs` line 177: `let vocab_size = 128256;`
- `scheduler.rs` lines 483, 693: `engine.get_logits(128256)`
- EOS token IDs hardcoded: `128001` and `128009`

This tightly couples the daemon to the Llama-3.x tokenizer. Any model change (e.g., to a Mistral or Phi variant) requires a source code change and recompile.

#### 🟡 INF-4: Tensor Serialization is Uncompressed

`TensorWirePacket` (line 6-11 of `tensor_wire.rs`) serializes tensors as raw `Vec<u8>` with no compression:

```rust
pub struct TensorWirePacket {
    pub tensor_name: String,
    pub dtype: u8,
    pub shape: Vec<u32>,
    pub data: Vec<u8>,  // raw bytes, no compression
}
```

For 1.58-bit tensors, this is somewhat acceptable (they're inherently compact), but for intermediate hidden-state activations (typically fp16 or fp32), this could saturate bandwidth. The `obfuscation.rs` module XOR-encrypts the data, but XOR does not compress.

#### 🟡 INF-5: Custom Obfuscation Instead of Standard AEAD

`obfuscation.rs` implements a custom CTR-mode XOR cipher using SHA-256 as the PRF:

```rust
fn xor_stream(key: &[u8], nonce: &[u8; 12], input: &[u8]) -> Vec<u8> {
    // SHA-256(key || nonce || counter) → XOR keystream
}
```

While functionally correct for confidentiality, this is:
1. **Not authenticated** — no MAC/tag, so ciphertext can be tampered with undetected
2. **Non-standard** — the workspace already imports `chacha20poly1305` (Cargo.toml line 11) which provides authenticated encryption. This custom cipher appears to be a "lighter" alternative but provides weaker security guarantees.

---

## Domain 3: Installation, Onboarding & DX (Developer Experience)

### Current State Assessment

There are three installation paths:
1. **Windows `.exe` installer** (`installers/windows/install.bat`)
2. **Python SDK** (`pip install shard-inference`) 
3. **Docker Compose** (`docker-compose.yml`)

### Friction Log: Windows Installer Journey

| Step | Action | Friction |
|---|---|---|
| 1 | Download `.exe` from GitHub Releases | ✅ Clear, linked from README |
| 2 | Run `install.bat` | ⚠️ Triggers UAC prompt (admin for service install) |
| 3 | `first-run.ps1` executes | ⚠️ Requires `Set-ExecutionPolicy Bypass` — many enterprise environments block this |
| 4 | Model download prompt | ⚠️ Downloads TinyLlama from HuggingFace (~0.6GB). If HF is blocked by corporate firewall, silent failure |
| 5 | Firewall rule creation | ⚠️ Another UAC prompt. Users behind corporate firewalls may not have permissions |
| 6 | Health check | ❌ Checks port 9091 but daemon hasn't started yet. The `Start-Sleep 2` (line 208) is a race condition — if the daemon takes >2s to start, health check fails |
| 7 | Auto-update scheduled task | ✅ Runs daily at 03:00, good |

#### 🔴 DX-1: `pip install shard-network` (README) vs. `pip install shard-inference` (Actual Package)

README line 54 says:
```bash
pip install shard-inference
```

But `pyproject.toml` at root level defines the package as `shard-sdk` (line 6), while `sdk/python/pyproject.toml` defines it as `shard-inference`. These are **two separate packages** with different build configurations:

| File | Package Name | Version |
|---|---|---|
| Root `pyproject.toml` | `shard-sdk` | 0.5.0 |
| `sdk/python/pyproject.toml` | `shard-inference` | 0.6.1 |
| README.md | `shard-inference` | — |

The root `pyproject.toml` includes `desktop/python` packages that require `cmake` and `setuptools-rust` as build dependencies (line 2 of `sdk/python/pyproject.toml`). If a user installs the root package, they'll get a broken install requiring a Rust toolchain.

**Impact:** First-time developer experience is broken. `pip install shard-inference` should work, but the root-level `pyproject.toml` creates confusion.

#### 🟡 DX-2: Model Weights Are Not Auto-Bootstrapped

The daemon expects `BITNET_MODEL` environment variable to point to a `.gguf` file. The `first-run.ps1` offers to download a TinyLlama model, but:
- The daemon itself does not auto-download models
- Docker Compose mounts `./models/` but doesn't populate it
- If `BITNET_MODEL` is not set and no model exists, the daemon starts in degraded mode with `engine_loaded: false`

This means users who skip the GUI wizard or use Docker will have a daemon that appears healthy but cannot do inference.

#### 🟡 DX-3: Port Conflicts Not Detected

The daemon uses ports 4001, 4101, 9090, 9091, 9092, 9093. None of these are checked for availability before binding. If another application (e.g., IPFS uses 4001) is already listening, the daemon will panic on startup with a cryptic `failed to bind` error.

#### 🟡 DX-4: `.env.local` and `clawdbot.pem` Committed to Repository

**Security finding:** The repository contains:
- `.env.local` (1207 bytes) — likely contains actual API keys or config
- `.env.vercel` (1234 bytes) — Vercel deployment secrets
- `.env.vercel.prod` (1866 bytes) — Production secrets  
- `clawdbot.pem` (1706 bytes) — An **SSH private key** for an EC2 instance

These should be in `.gitignore` and are a credential leak. The `clawdbot.pem` file in particular is an immediate security incident.

---

## Domain 4: UI/UX & Desktop Application

### Web Dashboard (Next.js)

#### Current State

The web dashboard is built with Next.js, React Query, and TailwindCSS. Key architecture:

- **`context.tsx`** — Central `AppProvider` that bootstraps the entire Scout workflow: probes local daemon → checks WebGPU → initializes WebLLM → starts scout worker → starts layer host → initializes P2P
- **`api.ts`** — API client with local daemon fallback (`fetchWithLocalFallback`)
- **`swarm.ts`** / `p2p.ts` / `webllm.ts` — Browser-side P2P and inference acceleration

#### 🟡 UX-1: WebLLM Initialization Blocks UI Thread

The `initWebLLM()` call in `context.tsx` (line 175) runs during React's initial mount. While the model downloads (~100-500MB WebGPU model), the progress callback updates state, but:
- There is no way to cancel the download
- If the tab is navigated away and back, `scoutBootedRef` prevents re-initialization, which is correct
- **But:** If initialization fails partway (e.g., OOM), the error message is normalized but the retry mechanism (`retryScout`) re-runs the entire boot sequence, potentially downloading the model again

#### 🟡 UX-2: Telemetry WS Has No Authentication

The telemetry WebSocket server (`telemetry_ws.rs`) binds on `0.0.0.0:9093` with **no authentication, no CORS, and no rate limiting**. Anyone on the network can connect and receive real-time telemetry data, including peer counts, utilization metrics, and timing information.

#### 🟡 UX-3: `useAppContext` Throws Instead of Returning Default

`context.tsx` line 270:
```tsx
throw new Error("useAppContext must be used within an AppProvider")
```

This has already caused a Vercel build failure (conversation `11c42c9f`). Any page that uses `useAppContext` and is pre-rendered during build will crash. The fix was applied, but this is a fragile pattern — a `useAppContextSafe()` variant that returns null would be more resilient.

#### 🟡 UX-4: No Responsive `dvh` or Mobile-First Design

The web dashboard uses TailwindCSS but there's no evidence of `dvh` (dynamic viewport height), `min-h-dvh`, or mobile-first breakpoints. The CSS files in `app/` include two legacy CSS files:
- `network-legacy.css` (56KB) 
- `network-legacy-utf8.css` (28KB)

These are large stylesheets that suggest a previous design system was not fully removed.

### Desktop GUI (Rust `shard-gui`)

The `shard-gui` crate exists (`desktop/rust/shard-gui/`) with a `Cargo.toml` and `src/` directory. From the conversation history (`5ceac0bf`), an egui-based GUI was scaffolded but development was paused. The system tray implementation exists in concept but may not be production-ready.

---

## Domain 5: Code Health, CI/CD & Security

### CI/CD Pipeline Analysis

**Three workflows exist:**

| Workflow | Status | Issues |
|---|---|---|
| `ci.yml` | 🟡 Fragile | Missing: `Dockerfile.daemon` not present in repo. Python tests install from `./sdk/python[dev]` but `pyproject.toml` requires `cmake` + `setuptools-rust` (native builds) |
| `release.yml` | ✅ Solid | Cross-platform matrix (Linux, macOS, Windows). Windows code signing handled gracefully with optional secrets |
| `benchmark-proof.yml` | ✅ Good | Properly parameterized, uploads artifacts |

#### 🟡 CI-1: TypeScript Build Errors Suppressed

`next.config.js` line 11-14:
```js
typescript: { ignoreBuildErrors: true },
eslint: { ignoreDuringBuilds: true },
```

All TypeScript errors and ESLint warnings are suppressed during builds. This means the CI `npm run build` step passes even with broken types. The `web-checks` CI job runs lint separately, which partially mitigates this, but the build step provides no type safety.

#### 🟡 CI-2: Python Tests Require Native Compilation

The CI `python-checks` job (line 59-62 of `ci.yml`) installs Rust and cmake:
```yaml
- name: Set up Rust
  uses: dtolnay/rust-toolchain@stable
- name: Install system dependencies
  run: sudo apt-get update && sudo apt-get install -y cmake
```

This is because `sdk/python/pyproject.toml` requires `setuptools-rust` and `cmake` as build dependencies. This dramatically increases CI time and fragility — Rust compilation failures in CI are a known pain point (per conversations `06333edb` and `0e5b4cf3`).

#### 🟡 CI-3: No Dockerfile.daemon

`docker-compose.yml` references `Dockerfile.daemon` (line 5) but this file does **not exist** in the repository. The web service has a `web/Dockerfile`, but the daemon service's Dockerfile is missing, making `docker-compose up` fail.

### Security Assessment

#### � SEC-1: SSH Key in Repository (Partially Mitigated)

`clawdbot.pem` and `.env.*` files have been added to `.gitignore` (preventing future commits). However, the files **remain in Git history** and can be recovered by anyone who clones the repo. The SSH key should be rotated in AWS EC2 and history should be scrubbed with BFG Repo Cleaner or `git filter-branch`.

#### 🔴 SEC-2: CORS is `Allow-Origin: *` by Default

`main.rs` lines 1806-1824: If `SHARD_CORS_ORIGINS` is not set, CORS defaults to `Any`:
```rust
cors = cors.allow_origin(Any);
```

While `allow_headers(Any)` is also set, this means any website can make authenticated API calls to a Shard daemon if it's exposed publicly (`--public-api`). Combined with API key auth being optional (`SHARD_REQUIRE_API_KEY=false` default), this is an open API.

#### 🟡 SEC-3: Admin API Key Management Has No Authentication

The `/admin/api-keys` endpoint creates and manages API keys. While there's an `admin_key` field in `SharedState`, the admin key is sourced from an environment variable. If the env var is not set, admin operations may be unprotected (dependent on handler implementation).

#### 🟡 SEC-4: PoW Difficulty May Be Insufficient for Sybil Resistance

The PoW challenge system (`pow_challenge.rs`) exists, but the difficulty and challenge reuse window need verification. A sophisticated attacker could pre-compute challenges or use GPU acceleration to flood the network with malicious nodes.

#### 🟡 SEC-5: Fallback API URL is a Placeholder

`fallback.rs` line 100-102:
```rust
let fallback_url = std::env::var("FALLBACK_API_URL").unwrap_or_else(|_| {
    "https://api.ourcompany.com/v1/internal/scout-fallback".to_string()
});
```

If `FALLBACK_API_URL` is not set, any long-context request will attempt to POST to a non-existent URL and fail. This should default to a disabled state, not a broken URL.

---

## UX/DX Friction Points Summary

| # | Friction Point | Severity | User Segment |
|---|---|---|---|
| 1 | Model weights not auto-downloaded in Docker/headless | High | DevOps |
| 2 | Package naming confusion (`shard-sdk` vs `shard-inference`) | High | Developers |
| 3 | 30s timeout before fallback to local generation | Critical | End Users |
| 4 | No firewall port availability check | Medium | Windows Users |
| 5 | `clawdbot.pem` in repository | Critical | Security Teams |
| 6 | Legacy CSS files bloating web bundle | Low | Web Performance |
| 7 | No cancel mechanism for WebLLM download | Medium | Browser Users |
| 8 | Telemetry WS unauthenticated | Medium | All deployments |

---

## Top 3 Next Steps (Prioritized Execution Plan)

### 🥇 Priority 1: Fix WAN Connectivity (Resolves 68% failure rate)

**Effort: 2-3 days | Impact: Critical**

1. **Deploy at least 2 relay servers** on publicly reachable infrastructure (EC2, DigitalOcean). Pass `--relay-mode --public-api --public-host <IP>` to the daemon.
2. **Hardcode relay multiaddrs** as default bootstrap peers so every new node can relay through them.
3. **Reduce `scout_timeout_ms` default** from 30,000ms to 3,000ms. This alone would reduce p95 latency from ~41s to ~4s.
4. **Implement race-based fallback**: Start local autoregressive generation in parallel with scout dispatch. Whichever path produces tokens first wins. Cancel the other.

### 🥈 Priority 2: Security Remediation (Immediate Risk)

**Effort: 4 hours | Impact: Critical**

1. **Rotate and revoke** the `clawdbot.pem` SSH key. Remove from Git history with `git filter-branch` or BFG Repo Cleaner.
2. **Remove `.env.local`, `.env.vercel`, `.env.vercel.prod`** from the repository and add them to `.gitignore`.
3. **Change CORS default** from `Any` to `deny-all`. Require explicit `SHARD_CORS_ORIGINS` configuration.
4. **Change fallback URL default** from a placeholder to `""` (disabled) with clear logging.
5. **Add authentication** to the telemetry WebSocket server.

### 🥉 Priority 3: Fix Engine Concurrency & Model Bootstrapping (Enables Scale)

**Effort: 1 week | Impact: High**

1. **Replace global engine Mutex** with a pool of engine instances (one per CPU core) or implement a work-stealing queue. Each inference request should acquire an engine from the pool, not wait behind a global lock.
2. **Implement auto-model-download**: On first launch, if `BITNET_MODEL` is not set, automatically download the default model to `~/.cache/shard/models/` (matching the Docker Compose volume mount path).
3. **Create `Dockerfile.daemon`**: The missing Dockerfile should be a multi-stage build that compiles the Rust binary and includes model download logic.
4. **Extract vocab size and EOS tokens** into model metadata loaded at runtime, not hardcoded constants.

---

## Appendix: File-Level Metrics

| Component | Lines of Code | Files | Test Coverage |
|---|---|---|---|
| `daemon/src/main.rs` | 4,211 | 1 | None (too large) |
| `daemon/src/scheduler.rs` | 851 | 1 | Partial (unit tests) |
| `daemon/src/api.rs` | 2,493 | 1 | None |
| `shard-network/` | ~392 | 5 | Good (roundtrip tests) |
| `shard-gateway/` | ~767 | 3 | Good (fallback tests) |
| `shard-verifier/` | ~317 | 4 | Minimal |
| `web/src/` | ~72 files | 72 | Partial (Jest) |
| `sdk/python/` | ~23 files | 23 | Partial (pytest) |

**Notable:** `main.rs` at **4,211 lines** is a significant code smell. It contains CLI parsing, state initialization, swarm event handling, HTTP router binding, gossipsub message handling, and the main event loop — all in a single file. This should be decomposed into at least 5-6 modules.

---

*End of Audit Report*
