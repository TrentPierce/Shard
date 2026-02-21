# Shard Refactor Plan (Post-Audit, Incremental)

## Preconditions
Audit artifacts are now complete:
- `ARCHITECTURE.md`
- `SYSTEM_FLOW.md`
- `CRYPTO_TRUST_MODEL.md`

No major structural refactor should start without preserving behavior through staged extraction and tests.

## Guiding Constraints
- Preserve Ed25519 as mandatory node identity.
- No token/blockchain/wallet economics scope.
- Rename identity-facing "wallet" semantics to node identity naming.
- Prioritize deterministic behavior and verifiable trust over feature breadth.

## Phase A: Stabilize Current Reality (Short, blocking)

### A1. Reconcile repo/docs/deploy mismatch
Tasks:
1. Remove or clearly deprecate stale Python API assumptions in active docs and scripts.
2. Fix broken root `Dockerfile` references (or retire it if daemon-only image is canonical).
3. Align compose monitoring scrape target with real runtime service.

Acceptance:
- `README.md` and deploy docs describe the actual runnable topology.
- `docker compose up` path is internally consistent.

### A2. Baseline test reliability
Tasks:
1. Keep Rust tests green as baseline.
2. Fix web test runtime by mocking/stubbing Tauri internals in Jest setup.
3. Add CI step to fail on web test/runtime errors.

Acceptance:
- CI passes with deterministic local reproduction.

## Phase B: Extract Clean Boundaries in Rust
Target module layout under `desktop/rust/src`:
- `gateway/`
- `scheduler/`
- `execution_nodes/`
- `verification/`
- `network/` (existing expanded)
- `metrics/`
- `identity/` (from `crypto/identity` + signing helpers)
- `common/`

### B1. Identity + Common first
Tasks:
1. Move identity primitives into `identity/`.
2. Introduce signed envelope primitives in `common/` (canonical payload hash, nonce, timestamp).
3. Rename APIs/types: `wallet` -> `node_identity`/`node_public_key`.

Acceptance:
- No behavior regressions in key loading/backup.
- Compile and tests pass.

### B2. Gateway extraction
Tasks:
1. Move HTTP route handlers and request validation out of `main.rs` into `gateway/`.
2. Keep route contract stable initially.
3. Add integration tests for gateway request validation and idempotency keys.

Acceptance:
- `main.rs` reduced to wiring/bootstrap.
- Gateway tests cover normal + malformed + duplicate requests.

### B3. Scheduler extraction
Tasks:
1. Create explicit scheduler trait and implementation.
2. Move penalty, race, and dispatch decision logic into `scheduler/`.
3. Add weighted selection inputs: load, historical latency, reliability, capability, identity reputation.

Acceptance:
- Deterministic selection under fixed seed/input.
- Unit tests for weighting and tie-break behavior.

### B4. Verification extraction
Tasks:
1. Move result verification and acceptance policy to `verification/`.
2. Enforce signature verification gate before accepting results.
3. Define clear rejection reasons and telemetry events.

Acceptance:
- Unsigned/invalidly signed results are rejected.
- Verification path test matrix covers valid/invalid/replay/expired.

## Phase C: Signed Control/Data Plane (Zero-Trust enforcement)

### C1. Signed node lifecycle
Tasks:
1. Signed registration message at startup.
2. Signed heartbeat every configurable interval.
3. Signed deregistration on shutdown.

Acceptance:
- Node health table keyed by Ed25519 public key.
- Missing heartbeat marks node unhealthy deterministically.

### C2. Signed work/result protocol
Tasks:
1. Signed work dispatch envelope.
2. Signed scout result envelope.
3. Replay protection (nonce/timestamp window).

Acceptance:
- Signature verification required before queue acceptance.
- Replay attempts rejected and counted.

## Phase D: Observability and Metrics

### D1. Prometheus-native metrics
Implement counters/gauges/histograms for:
- tokens processed
- tokens offloaded
- verification fallback rate
- task failure rate
- node uptime
- node latency
- scheduler decision latency
- end-to-end latency (p50/p95/p99)
- queue depth
- active node count
- signature verification failures
- identity auth failures

Acceptance:
- `/metrics` endpoint exposed by gateway.
- Grafana dashboard queries align with actual metric names.

### D2. Metrics persistence and aggregation
Tasks:
1. Local SQLite sink (dev).
2. Postgres sink (prod option).
3. Periodic node metric reports aggregated at gateway.

Acceptance:
- Consistent schema + migration scripts.
- Dashboard can read live + historical aggregates.

## Phase E: Reliability + Fallback

### E1. Deterministic fallback contract
Tasks:
1. Timeout policy for scout responses.
2. Automatic fallback execution on shard node.
3. Idempotent request handling with durable request IDs.

Acceptance:
- No duplicate side effects under retries.
- Explicit fallback metrics and signed fallback events.

### E2. Reputation model hardening
Tasks:
1. Identity-bound reputation (pubkey keyed).
2. Historical latency + success/failure rolling windows.
3. Scheduler weight integration.

Acceptance:
- Reputation survives process restart.
- Reputation cannot be transferred by changing transient scout IDs.

## Phase F: Deployment + Cluster Controls

### F1. Containerization and config simplification
Tasks:
1. Ensure all services are containerized for current architecture.
2. Single YAML config with env override support.
3. Add resource control knobs (`max_cpu_usage`, `max_gpu_usage`, `idle_only_mode`, `load_threshold_cutoff`).

Acceptance:
- `docker-compose` cluster mode works with multiple node roles.

### F2. Demo cluster readiness
Tasks:
1. 1 gateway + >=3 shard + >=5 scout deploy profile.
2. Signed identities for all nodes.
3. Live health dashboard with auto-refresh.

Acceptance:
- Stable multi-node demo script with quick start docs.

## Phase G: Benchmark + Cost Tooling

### G1. `shard-load-test` CLI
Capabilities:
- 100/1k/10k concurrency
- throughput, latency, offload%, failure rate
- signature validation overhead
- JSON + Markdown report output

### G2. Benchmark suite + cost estimation
Tasks:
1. Baseline vs distributed benchmark modes.
2. Timestamped report storage in `/benchmarks`.
3. Cloud GPU cost estimation module surfaced in dashboard.

Acceptance:
- Reproducible benchmark command set with saved artifacts.

## Testing Strategy (target >=70% core coverage)
- Unit tests: identity signing/verification, scheduler weighting, replay protection, verification policy, fallback logic.
- Integration tests: gateway -> scheduler -> execution node -> verification with signed messages.
- Simulated node tests: heartbeat loss, invalid signature, slow node, replay attempts, partial partition.
- Concurrency tests: high fanout and race behavior under contention.

## CI/CD Upgrade Plan
1. Keep existing Rust/web jobs.
2. Add coverage reporting for core Rust modules.
3. Add integration test job with ephemeral multi-node setup.
4. Add lint/format checks for all active stacks.
5. Add benchmark smoke job on manual trigger.

## Immediate Next Implementation Slice (recommended)
1. Create module skeletons and move only type definitions/interfaces first.
2. Implement signed envelope type and verification middleware.
3. Convert one route flow end-to-end (`broadcast-work` + result submission) to signed enforcement.
4. Add integration tests for this single signed flow before broader migration.

This sequence minimizes blast radius while establishing the trust and boundary model required for the full roadmap.
