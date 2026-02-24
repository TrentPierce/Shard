Execution Backlog (Epics, Tickets, Acceptance Criteria, Sequencing)

  ## 1. Epic List

  1. E1 Core Protocol Contract Stabilization
  2. E2 Speculative Decoding Production Loop
  3. E3 Mesh Reliability and Self-Healing
  4. E4 Security Hardening and Trust Controls
  5. E5 Windows Node Productization
  6. E6 Scout Web Contribution Hardening
  7. E7 Unified Versioning and Release Governance
  8. E8 Observability, SLOs, and Operations
  9. E9 Documentation and Developer Experience
  10. E10 Model Upgrade Framework (Stronger Verifier)

  ———

  ## 2. Ticket Backlog (Prioritized)

  ## E1 Core Protocol Contract Stabilization

  1. P0-T1 Create canonical API/protocol spec (/v1/chat/completions, /v1/scout/work, /v1/scout/draft, signed envelope).

  - Owner: Backend Lead
  - Complexity: M
  - Acceptance: Spec merged; JSON schema files in repo; contract tests exist and pass in CI.

  2. P0-T2 Align inference-mode header semantics across web/daemon (distributed|standard|speculative contract).

  - Owner: Web Lead + Backend Lead
  - Complexity: S
  - Acceptance: Integration test proves speculative branch activates when UI selects distributed mode.

  3. P0-T3 Define draft payload contract (token IDs required or server tokenization guarantee).

  - Owner: ML Infra + Backend Lead
  - Complexity: M
  - Acceptance: End-to-end test confirms non-empty verifier-consumable draft tokens per request.

  ## E2 Speculative Decoding Production Loop

  1. P0-T4 Wire real accept/reject path with measurable acceleration metrics.

  - Owner: Backend Lead
  - Complexity: L
  - Acceptance: Metrics expose acceptance rate, reject rate, speedup; regression tests pass.

  2. P1-T5 Integrate probabilistic MatMul spot-check into live verifier decision path.

  - Owner: ML Infra
  - Complexity: L
  - Acceptance: Configurable sample-rate/tolerance in runtime; fraud/tamper tests fail as expected.

  3. P1-T6 Add fallback policy tests for timeout/partial-generation behavior.

  - Owner: Backend + QA
  - Complexity: M
  - Acceptance: Scenario suite covers scout timeout, disconnected scout, long-context fallback.

  ## E3 Mesh Reliability and Self-Healing

  1. P0-T7 Peer health scoring and reconnect policy hardening.

  - Owner: Networking Lead
  - Complexity: M
  - Acceptance: Churn tests show automatic recovery without manual intervention.

  2. P1-T8 Bootstrap registry implementation (replace stub registration response).

  - Owner: Networking Lead
  - Complexity: M
  - Acceptance: Stable bootstrap candidates persisted and queryable; no-op registration removed.

  3. P1-T9 Capability-aware scheduling (latency/load/model compatibility).

  - Owner: Backend + Scheduler Owner
  - Complexity: M
  - Acceptance: Scheduler selects nodes using live capability reports; decision logs auditable.

  ## E4 Security Hardening and Trust Controls

  1. P0-T10 Enforce PoW verification on ingress routes.

  - Owner: Security Engineer + Backend Lead
  - Complexity: M
  - Acceptance: Unverified peers rejected from draft/work routes; bypass paths removed.

  2. P0-T11 Enforce auth policy consistency (SHARD_REQUIRE_API_KEY, admin key, private route).

  - Owner: Security Engineer
  - Complexity: M
  - Acceptance: Policy matrix documented; tests prove each mode behavior.

  3. P1-T12 Replay/nonce/signature abuse test suite.

  - Owner: QA + Security
  - Complexity: M
  - Acceptance: Negative tests for replay/tamper pass in CI.

  ## E5 Windows Node Productization

  1. P0-T13 Build Windows installer (MSI/EXE) with service mode (`unsigned-supported` preview until trusted certs are provisioned).

  - Owner: Desktop Lead
  - Complexity: L
  - Acceptance: Silent install/uninstall supported; service starts and reports healthy.

  2. P0-T14 First-run onboarding wizard (identity, model download, connectivity, firewall prompts).

  - Owner: Desktop UX + Desktop Lead
  - Complexity: M
  - Acceptance: New user can join and contribute in <10 minutes on clean machine.

  3. P1-T15 Auto-update channel with rollback.

  - Owner: Release Engineer
  - Complexity: L
  - Acceptance: Canary and stable channels; rollback validated in staging.

  ## E6 Scout Web Contribution Hardening

  1. P1-T16 Single scout path cleanup (remove duplicate/unused scout implementations).

  - Owner: Web Lead
  - Complexity: M
  - Acceptance: One canonical scout runtime path; dead code removed; tests green.

  2. P1-T17 Browser capability profile + clear contribution status UX.

  - Owner: Web Lead
  - Complexity: S
  - Acceptance: User sees exact reason if not contributing; telemetry reflects contribution state.

  3. P1-T18 Scout reliability guardrails (timeouts, retries, queue discipline).

  - Owner: Web + Backend
  - Complexity: M
  - Acceptance: Scout contribution remains stable under transient network issues.

  ## E7 Unified Versioning and Release Governance

  1. P0-T19 Enforce single-source version propagation from root VERSION.

  - Owner: Release Engineer
  - Complexity: S
  - Acceptance: CI fails on mismatch across Rust/web/Python/Node/installers.

  2. P0-T20 Runtime version surfacing in health endpoints/UI/SDK.

  - Owner: Backend + Web + SDK Owners
  - Complexity: S
  - Acceptance: Version visible and identical across all surfaces for same release.

  3. P1-T21 Release manifest and artifact signing standardization.

  - Owner: Release Engineer
  - Complexity: M
  - Acceptance: All published artifacts signed and traceable to git tag + VERSION.

  ## E8 Observability, SLOs, and Operations

  1. P0-T22 Define SLIs and SLOs (availability, latency, acceptance, throughput).

  - Owner: SRE Lead
  - Complexity: S
  - Acceptance: SLO doc merged; dashboards and alerts mapped to SLIs.

  2. P1-T23 Production dashboards and alerts hardening (Prometheus/Grafana).

  - Owner: SRE
  - Complexity: M
  - Acceptance: Alert runbook links; paging thresholds validated in load tests.

  3. P1-T24 Incident response and on-call runbooks.

  - Owner: SRE + Security
  - Complexity: S
  - Acceptance: Tabletop incident drill completed with documented outcomes.

  ## E9 Documentation and Developer Experience

  1. P0-T25 Rebuild docs baseline (architecture.md, api.md, deployment.md, security.md, operations.md, contributing.md).

  - Owner: Tech Writer + Component Owners
  - Complexity: M
  - Acceptance: Every endpoint/flow documented and verified against tests.

  2. P0-T26 Remove broken README links and stale claims immediately.

  - Owner: Tech Writer
  - Complexity: S
  - Acceptance: README links resolve; claims map to implemented behavior.

  3. P1-T27 Add architecture decision records (ADRs) for protocol/security/scheduler.

  - Owner: Architect
  - Complexity: S
  - Acceptance: ADR index in docs with accepted decisions and rationale.

  ## E10 Model Upgrade Framework (Stronger Verifier)

  1. P1-T28 Model interface abstraction and compatibility checks.

  - Owner: ML Infra
  - Complexity: M
  - Acceptance: Verifier model swap does not require gateway/protocol rewrites.

  2. P2-T29 Canary rollout for stronger model with auto-rollback.

  - Owner: ML Infra + SRE
  - Complexity: L
  - Acceptance: Traffic-splitting and rollback based on quality/latency thresholds.

  3. P2-T30 Publish model compatibility matrix (draft/verifier pairs).

  - Owner: ML Infra
  - Complexity: S
  - Acceptance: Matrix consumed by scheduler and release docs.

  ———

  ## 3. Sequenced Delivery Plan (By Sprint Block)

  1. Block A (Weeks 1-2): P0-T1, T2, T3, T19, T25, T26

  - Goal: Contract correctness + version governance + docs baseline.

  2. Block B (Weeks 3-4): P0-T4, T7, T10, T11, T22

  - Goal: Core loop reliability and security enforcement.

  3. Block C (Weeks 5-6): P0-T13, T14, T20 + P1-T16

  - Goal: Windows onboarding and unified product identity.

  4. Block D (Weeks 7-8): P1-T5, T8, T9, T23, T24

  - Goal: Deep mesh scaling, observability, and operational hardening.

  5. Block E (Weeks 9-10): P1-T15, T18, T21, T27, T28

  - Goal: Release safety, DX maturity, model abstraction.

  6. Block F (Weeks 11-12): P2-T29, T30

  - Goal: Stronger-model canary and rollout readiness.

  ———

  ## 4. Ownership Map

  1. Backend Lead: T1 T2 T3 T4 T6 T9 T10 T11 T20
  2. Networking Lead: T7 T8
  3. ML Infra Lead: T3 T5 T28 T29 T30
  4. Web Lead: T2 T16 T17 T18 T20
  5. Desktop Lead: T13 T14
  6. Security Engineer: T10 T11 T12
  7. Release Engineer: T15 T19 T21
  8. SRE Lead: T22 T23 T24
  9. Tech Writer: T25 T26 T27
  10. QA Lead: T6 T12

  ———

  ## 5. Minimum “Go-Live Enterprise” Gate

  1. All P0 tickets completed.
  2. Contract, integration, and chaos tests passing in CI.
  3. Windows installer validated on fresh hosts (signed when trusted certs are available; otherwise explicitly marked `unsigned-supported` preview).
  4. Version parity verified automatically across all components.
  5. SLO dashboards and alert runbooks operational.
  6. Security controls (PoW/auth/replay) enforced in production paths.

  ---

  ## 6. Execution Status (2026-02-24)

  - P0-T1: `Done` (canonical schemas + contract tests in `tests/contract_protocol_test.py`)
  - P0-T2: `Done` (inference-mode semantics aligned; `distributed` maps to speculative; scheduler tests added)
  - P0-T3: `Done` (draft payload enforces non-empty verifier-consumable tokens via tokens-or-tokenization contract)
  - P0-T4: `Done` (accept/reject/speedup speculative metrics added and surfaced in metrics summary and Prometheus)
  - P0-T7: `Done` (reconnect and bootstrap-failure recovery logic extracted and covered by unit tests)
  - P0-T10: `Done` (PoW enforcement added to `/v1/scout/work` and `/v1/scout/draft` runtime paths)
  - P0-T11: `Done` (`SHARD_REQUIRE_API_KEY` + private route auth matrix enforced; auth-policy tests added)
  - P0-T13: `Done` (service mode, silent install/uninstall, rollback-safe backup, and first-run onboarding are productionized; Windows releases are officially `unsigned-supported` preview when trusted signing certs are unavailable. Follow-up: enable trusted Authenticode signing and promote Windows channel from preview to fully signed GA.)
  - P0-T14: `Done` (interactive Windows installer now triggers GUI first-run wizard with model/firewall/health/bootstrap checks; silent install remains non-interactive)
  - P0-T19: `Done` (root `VERSION` propagation + `scripts/verify_versions.py` mismatch gate now covers all 8 Rust library crates; `scripts/sync_versions.py` now updates crate manifests to prevent drift)
  - P0-T20: `Done` (runtime version surfaced in web health payload, web footer, and SDK runtime fields; web runtime hotfixes added for hosted/mobile safety: chat auto-falls back to non-streaming when SSE reader is unavailable, localhost daemon fallback is gated to local/Tauri contexts, topology/heartbeat probes are host-aware with short timeouts, dashboard telemetry now derives shard count from health+peer signals without simulated defaults, app-shell hydration risk was reduced by removing top-level libp2p imports from global startup, CSP script policy now permits Next.js inline bootstrap scripts to prevent hydration breakage, and chat responses now strip leaked model control tokens in both daemon and web output paths)
  - P0-T22: `Done` (SLI/SLO definition and operations mapping documented)
  - P0-T25: `Done` (docs baseline rebuilt: architecture/api/deployment/security/operations/contributing/versioning/model-upgrade)
  - P0-T26: `Done` (README and web footer stale/broken links fixed)

  ## 7. P1 Status (2026-02-24)

  - P1-T5: `Done` (live draft path now enforces configurable matmul spot-check; tamper tests added)
  - P1-T6: `Done` (timeout/cooldown and long-context fallback policy tests added)
  - P1-T8: `Done` (bootstrap registry persisted and queryable via `/v1/system/bootstrap`)
  - P1-T9: `Done` (scheduler decisions now logged with auditable inputs via `/v1/system/scheduler-decisions`)
  - P1-T12: `Done` (replay/nonce/signature abuse tests expanded in `shard-common` and daemon replay tests; CI-covered via rust test job)
  - P1-T15: `Done` (stable/canary auto-update channel + rollback-safe updater implemented)
  - P1-T16: `Done` (single canonical scout work/draft path consolidated in `web/src/lib/scout-draft.ts`; duplicate browser scout path removed)
  - P1-T17: `Done` (browser capability profiling now drives explicit contribution status/reason in app context and start page UX; telemetry event emitted; WebLLM runtime initialization failures are normalized into actionable browser guidance, Windows Chrome WebGPU worker init now retries through direct-engine fallback for known `GPUDevice.lost`/FFI invocation failures, and CSP now allows WebLLM WASM runtime evaluation paths required for fallback initialization)
  - P1-T18: `Done` (scout submission queue discipline, bounded retries/backoff, timeout controls, and resilient polling retries implemented + tested)
  - P1-T21: `Done` (release manifest generation/verification and artifact signing workflow standardized)
  - P1-T23: `Done` (Prometheus alert rules + Grafana panel hardening with runbook mapping)
  - P1-T24: `Done` (incident/on-call runbooks and tabletop drill outcomes documented)
  - P1-T27: `Done` (ADR index and accepted protocol/security/scheduler records added)
  - P1-T28: `Done` (verifier model abstraction + compatibility checks exposed in runtime health)

  ## 8. P2 Status (2026-02-24)

  - P2-T29: `Done` (canary verifier rollout controller added with request traffic-splitting, runtime status endpoint, and auto-rollback on latency/quality threshold regressions)
  - P2-T30: `Done` (published draft/verifier compatibility matrix and enforced compatibility checks in scheduler/chat and layer scheduling paths)
