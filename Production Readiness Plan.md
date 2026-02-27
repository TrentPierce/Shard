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

  - Stable/canary channels with rollback; signed artifacts when trusted cert material is available. Windows remains explicitly `unsigned-supported` preview otherwise.

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
  2. Auto-update and release artifact governance (signed where certs are available; explicit unsigned preview designation on Windows when certs are absent).
  3. Load/scaling validation with published benchmark results.
  4. Canary model-upgrade framework.

  ———

  ## 7. Definition of Done (Enterprise-Ready Baseline)

  1. 99.9% control-plane uptime target with documented SLO.
  2. Reproducible release pipeline with rollback and explicit artifact trust state (signed where certs are available; Windows unsigned preview allowed with disclosure).
  3. One-click Windows install, <10 min time-to-contributing.
  4. Verified horizontal scaling: more online Shards measurably increases tokens/sec.
  5. Accurate docs that match shipped behavior and version.
  6. Security controls enforced in runtime, not only documented.

  ---

  ## 8. Execution Progress (2026-02-24)

  - Completed foundations: protocol schemas, contract tests, auth/PoW runtime enforcement, and version-governance CI checks.
  - Completed docs baseline: architecture, API, deployment, security, operations, contributing, versioning, model-upgrade strategy.
  - Completed runtime consistency work: distributed/speculative header contract and surfaced version in web runtime outputs.
  - Mesh reliability hardening now includes reconnect/bootstrap-failure recovery test coverage.
  - Windows productization hardening: interactive first-run GUI onboarding is implemented; Windows release channel is now formally classified as `unsigned-supported` preview when trusted cert secrets are unavailable.
  - P1 progress: spot-check verifier path, persisted bootstrap registry, scheduler decision audit trail, and observability runbooks are now implemented.
  - Release safety and model-upgrade readiness now include auto-update channels, release-manifest/signing standards, ADRs, and verifier-model compatibility abstraction.
  - Security abuse coverage expanded: replay/nonce/signature tamper tests now explicitly exercise negative cases in Rust unit suites.
  - Scout web hardening completed: duplicate scout runtime path removed, canonical draft/work path consolidated, and contribution-state UX + telemetry added.
  - Scout reliability guardrails completed: bounded queueing, timeout-aware retries, and resilient work polling backoff are active in the browser scout runtime.
  - Model rollout framework completed: canary traffic splitting with auto-rollback thresholds and operational reset endpoint now active in daemon runtime.
  - Compatibility governance completed: published draft/verifier matrix is now enforced by scheduler compatibility checks and surfaced in upgrade docs.
  - Web production UX hotfix completed: hosted mobile clients now avoid unsafe localhost fallback paths, chat degrades to non-stream completions when SSE streams are unavailable, and telemetry dashboards show backend truth instead of synthetic simulation defaults.

  ## 9. Post-Readiness Remediation Focus (2026-02-25)

  1. Runtime metrics correctness first:

  - Fix token accounting so runtime usage fields and aggregate counters represent real generated tokens.
  - Prevent false healthy/contributing state when model runtime is unavailable.

  2. Speculative proof before scaling claims:

  - Reactivate draft/verify flow so distributed mode emits non-zero speculative counters.
  - Add CI gate that fails when distributed runs show zero draft counters without explicit fallback reason.

  3. Reliability before benchmark storytelling:

  - Reduce EC2 endpoint timeout and success-rate regressions under benchmark concurrency.
  - Add readiness states (`ready`, `degraded`, `unavailable`) so operations reflects actual serving ability.

  4. Decentralization hardening:

  - Move bootstrap defaults to registry-seeded and TTL-pruned sources.
  - Validate mesh continuity under single-node outages without central dependency.

  5. Observability closure:

  - Add alerting for zero-token drift despite successful requests.
  - Track scout submit success and liveness as first-class SLI signals.

  ## 10. Remediation Progress (2026-02-27)

  - R0-T31 complete: daemon chat completion responses emit non-zero `usage` token counts and increment runtime token counters on generated output.
  - R0-T32 complete: runtime readiness is explicitly surfaced (`status`, `readiness_reason`, `ready_for_inference`) so nodes without loaded models are not presented as fully ready.
  - R0-T33 complete: repetitive output degeneration guard now halts repeated-token loops in both streaming and non-streaming generation, records detector counters, and triggers fallback/reset signaling.
  - R0-T34 complete: speculative draft/verify flow now consistently produces non-zero accepted speculative counters in live runs; wait/mailbox/idempotent lifecycle cleanup and queue/backpressure handling were hardened to keep work-id handoff deterministic under concurrency.
  - R0-T35 complete: benchmark/CI gate now fails distributed runs when speculative draft counters stay zero without an explicit fallback reason, preventing silent regressions.
  - R0-T38 partially complete: web scout/proxy reliability now includes transient-aware retries, jittered backoff, bounded failover budgets, stale-snapshot serving across health/peers/topology/metrics, and scout-route backend cooldown/load-shedding to prevent failure-amplification loops; EC2 distributed p95 latency is still above target and remains the primary open performance risk.
  - R1-T39 complete: tri-state readiness semantics are now propagated in web proxy responses (`health_state`: `ready`/`degraded`/`unavailable`) and consumed by dashboard telemetry state with explicit UI readiness badging; dashboard telemetry status transitions are covered by automated tests.
  - R1-T40 complete: dashboard total-token telemetry sources authoritative backend counters (`tokens_processed_total + tokens_offloaded_to_scouts_total`), with token-drift and scout-submit alert wiring now active and runbook thresholds documented.
  - Web proxy SLI completion: `/api/v1/chat/completions` now records proxy-side request outcome counters/rates (5xx/timeout/attempts), exports them in Prometheus format via `/api/metrics`, and pages on sustained 5xx-rate breaches while backend health-ready is true.
  - Bootstrap registry hardening: daemon bootstrap resolution now seeds from persisted bootstrap registry entries filtered by TTL/stability score, and stale registry entries are pruned with known-peer cleanup to reduce stale bootstrap dial loops.
  - Scout readiness correction complete: scheduler eligibility now keys off scout runtime capability (`webgpu` vs `wasm` fallback) instead of raw active browser sessions, eliminating fallback-only scout inflation in speculative wait-time calculations.
  - Metrics correctness fix complete: daemon metric surfaces now publish true p95 latency values (`p95_ms`) in both Prometheus `/metrics` and JSON `/metrics/summary` outputs.
  - Model alignment update: default scout/verifier model identity is now `meta-llama/Llama-3.2-1B` (with legacy alias normalization) so distributed speculative compatibility no longer depends on legacy `shard-hybrid`/`default-model` identifiers.
  - TODO-WEB-RECONNECT-01 complete: browser scout worker lifecycle persists across topology refresh/re-render cycles so open tabs auto-resume contribution after backend restart/deploy without manual refresh.
