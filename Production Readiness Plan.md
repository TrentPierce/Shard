Production Readiness Plan (Enterprise + Maintainability)

  ## 1. North-Star Outcomes

  1. Reliable end-to-end distributed inference (prompt -> scout drafts -> shard verify -> stream response).
  2. Deterministic versioning across daemon/web/SDK/installers.
  3. One-click Windows onboarding for Shard nodes.
  4. Hardened, observable mesh where capacity scales with online Shards.
  5. Controlled model upgrade path (stronger verifier model without network breakage).

  ———

  ## 2. Program Structure (4 Tracks in Parallel)

  ### Track A: Platform Reliability + Security

  1. Define supported architecture contract:

  - Transport/protocol versions.
  - API schemas for /v1/chat/completions, /v1/scout/work, /v1/scout/draft.
  - Signed envelope + nonce + PoW enforcement rules.

  2. Enforce security gates:

  - Require auth policy explicitly (SHARD_REQUIRE_API_KEY behavior fixed).
  - Enforce PoW verification in ingress routes (not just expose challenge endpoints).
  - Add rate-limits by identity + IP + wallet.

  3. Implement failure recovery:

  - Retry policy, peer health scoring, backoff, quarantine.
  - Reconnection and bootstrap trust model.

  4. Add chaos/fault tests:

  - Peer churn, packet delay/loss, shard drop mid-request, stale draft replay.

  ### Track B: Codebase Cleanup + Maintainability

  1. Refactor into clear bounded modules:

  - mesh, gateway, speculative, ledger, identity, ops.

  2. Remove dead/duplicated paths:

  - Eliminate parallel unused scout implementations.
  - Remove stale docs references and legacy routes.

  3. Add strict quality gates:

  - No ignored build errors.
  - Lint + typecheck + unit/integration/e2e tests required for merge.

  4. API compatibility testing:

  - Contract tests between daemon, web, Python SDK, Node SDK.

  ### Track C: Productization (Installers + UX)

  1. Windows first-class installer:

  - Signed MSI/EXE with service install, auto-start option, health checks, rollback.
  - Embedded first-run wizard: hardware probe, model download, port check, firewall rule.

  2. Seamless joining:

  - “Join network” flow with node identity generation, bootstrap sync, telemetry confirmation.

  3. Auto-update channel:

  - Stable/canary channels with signed artifacts and rollback.

  4. Scout UX hardening:

  - Browser capability detection + fallback path + clear contribution status.

  ### Track D: Documentation + Release Governance

  1. Replace doc sprawl with source-of-truth docs:

  - docs/architecture.md, docs/api.md, docs/deployment.md, docs/security.md, docs/operations.md.

  2. Versioned docs:

  - Every release tags docs with version and compatibility matrix.

  3. Runbooks:

  - Incident response, key rotation, node recovery, bootstrap rotation, model migration.

  4. Enterprise readiness artifacts:

  - Threat model, SLOs/SLIs, change management policy, support policy.

  ———

  ## 3. Versioning Strategy (Single Source of Truth)

  1. Keep one canonical VERSION file at repo root.
  2. CI enforces sync into:

  - Rust crates, web package(s), Python SDK, Node SDK, installers, release manifests.

  3. Runtime version visibility:

  - /health, /node/status, web footer, installer “About”, SDK client.version.

  4. Release pipeline:

  - Tag from canonical version only.
  - Block release if any component version mismatch is detected.

  ———

  ## 4. Mesh Hardening + Scaling Plan

  ## Phase 1: Make Scaling Real

  1. Fix speculative header/token contract mismatch across web <-> daemon.
  2. Ensure draft payloads are verifier-usable (token IDs or deterministic tokenizer bridge).
  3. Add real peer capability advertisement:

  - Model ID, layer range, throughput, latency, load.

  4. Scheduler uses capability + health + latency + trust score.

  ## Phase 2: Increase Throughput with More Shards

  1. Work partitioning strategy:

  - Layer routing / request sharding / queue-aware dispatch.

  2. Admission control:

  - Protect verifier under burst.

  3. Backpressure and fairness:

  - Prevent one noisy peer from starving others.

  4. Validate scaling:

  - Benchmark matrix for 1, 3, 5, 10+ Shards with scout mix; publish tokens/sec curve.

  ———

  ## 5. Stronger Model Upgrade Path

  1. Model abstraction layer:

  - Separate model interface from transport and scheduling.

  2. Compatibility matrix:

  - Draft model(s) vs verifier model versions.

  3. Canary rollout:

  - Small % traffic to new verifier model with quality/latency guardrails.

  4. Rollback policy:

  - Automatic rollback on SLO regression.

  5. Data-plane version negotiation:

  - Prevent mixed-node incompatibility during rolling upgrades.

  ———

  ## 6. Milestones (Suggested)

  ### Milestone 1 (0-30 days): Foundation Lockdown

  1. Fix contract mismatches in speculative loop.
  2. Enforce build/test gates, remove ignored errors.
  3. Restore accurate docs skeleton + missing referenced files.
  4. Make deploy artifacts valid (Dockerfile/flags/compose).

  ### Milestone 2 (30-60 days): Hardening + Installer Beta

  1. PoW enforcement + auth policy consistency.
  2. Windows installer beta with onboarding wizard and health checks.
  3. E2E test suite for full inference loop + peer churn.
  4. Unified version propagation + runtime version reporting.

  ### Milestone 3 (60-90 days): Enterprise Readiness

  1. SLO-backed observability dashboards + alerts.
  2. Auto-update and signed releases.
  3. Load/scaling validation with published benchmark results.
  4. Canary model-upgrade framework.

  ———

  ## 7. Definition of Done (Enterprise-Ready Baseline)

  1. 99.9% control-plane uptime target with documented SLO.
  2. Reproducible release pipeline with signed artifacts and rollback.
  3. One-click Windows install, <10 min time-to-contributing.
  4. Verified horizontal scaling: more online Shards measurably increases tokens/sec.
  5. Accurate docs that match shipped behavior and version.
  6. Security controls enforced in runtime, not only documented.

  ---

  ## 8. Execution Progress (2026-02-24)

  - Completed foundations: protocol schemas, contract tests, auth/PoW runtime enforcement, and version-governance CI checks.
  - Completed docs baseline: architecture, API, deployment, security, operations, contributing, versioning, model-upgrade strategy.
  - Completed runtime consistency work: distributed/speculative header contract and surfaced version in web runtime outputs.
  - In progress: Windows productization hardening (code paths present; signing and GUI wizard remain as follow-up items).
