> [!WARNING]
> **DEPRECATED**: This audit was generated on March 5, 2026. Prioritized items have been fixed or rendered obsolete. This file is retained for historical archive purposes only. Does not reflect current system state.

# Shard Network — Production Readiness Audit

**Date:** March 5, 2026
**Version Audited:** 0.6.5
**Auditor:** Automated deep-dive analysis across architecture, tests, security, UX, and deployment

---

## Part A: Current Status Report

### Release Readiness Grade: **62 / 100**

Shard is an ambitious, well-architected distributed inference network at pre-release maturity. The core data flow — Scouts draft, Verifiers validate, clients receive OpenAI-compatible responses — works end-to-end. Rust daemon, Next.js web frontend, Python SDK, benchmarks, CI/CD, Docker, Terraform, and multi-platform release pipelines all exist and function. However, several structural and operational issues must be resolved before a production launch.

### Top 5 Blockers Preventing Launch Today

| # | Blocker | Severity |
|---|---------|----------|
| 1 | **`lib.rs` is a 5,986-line god file** — The Rust daemon core puts virtually all logic (swarm event loop, state management, gossipsub handlers, consensus wiring, canary rollout, engine loading, telemetry, 30+ handler types) in a single file. This is the #1 maintainability and auditability risk. Any bug fix or feature addition is a high-risk merge conflict zone. | Critical |
| 2 | **Scout path tail latency is 3-10x worse than verifier-only** — The README's own benchmark snapshot shows 1-node-with-scouts p95 at 10,016 ms vs 4,523 ms without, and a 5.78% error rate. The 2-node-with-scouts p95 is 7,719 ms with 0.83% errors. Scout-assisted inference is the core value proposition; it must not degrade latency by 2-3x. | Critical |
| 3 | **Wildcard CORS on chat completions proxy** — `web/src/app/api/v1/chat/completions/route.ts` sets `Access-Control-Allow-Origin: *` on both streaming responses and OPTIONS preflight. In production this allows any origin to invoke the chat API from a browser, which is a credential-leaking and abuse vector. | High |
| 4 | **No end-to-end test suite** — There is one integration test (`test_failover.py`) that requires a pre-built binary and spawns 3 daemons. There are zero E2E tests for the web UI, zero tests for the Scout contribution flow, and zero tests for the overflow router's circuit breaker under real traffic. CI only runs `cargo test` + `jest --passWithNoTests`. | High |
| 5 | **42 `unwrap()` calls across Rust crates** — Production Rust code should almost never panic. `unwrap()` in the daemon, gateway rate limiter, and crypto crates can crash the process on unexpected input. Several are in hot paths like PoW challenge validation and race routing. | High |

### Honest Assessment

**What's strong:**
- Architecture is sound: modular Rust workspace with 8 crates, clean separation between daemon/gateway/crypto/ledger/metrics/network/scheduler/verifier
- Python SDK is well-typed with pydantic models, proper error hierarchy, and comprehensive resource coverage
- Overflow router with circuit breaker + SLA enforcer is production-grade pattern
- CI/CD pipeline covers Rust + Web + version sync check; release workflow builds Linux/macOS/Windows + publishes to PyPI
- Docker Compose with health checks, Prometheus, Grafana monitoring stack
- CSP headers, PoW gating, Sybil detection, KL-divergence verification all implemented
- Benchmark harness is thorough with distributed orchestrator, scenario runner, and comparison tools
- Documentation is extensive (17 docs covering architecture, deployment, API, SLA, versioning, enterprise VPC)

**What's weak:**
- Core Rust logic concentrated in 2 massive files (lib.rs + api.rs = 10,086 lines)
- Test coverage is thin: ~7 Python tests, ~11 web tests, Rust tests unknown but likely limited given the monolithic structure
- Legacy/dead files exist (app.tsx, swarm.ts re-exports, UTF-16 encoded CSS duplicate)
- No rate limiting on the web proxy layer (CORS wildcard + no auth = open relay risk)
- `lint-web` in Makefile silently swallows errors (`2>/dev/null || true`)
- GUI crate has 13 `unwrap()` calls but is user-facing software

---

## Part B: The Cleanup Hitlist

### Dead/Obsolete Files to Delete

| File | Reason |
|------|--------|
| `web/src/app.tsx` | Self-documents as "superseded by src/app/page.tsx" — empty re-export |
| `web/src/swarm.ts` | Re-export shim for `./lib/swarm` — all imports should point to `@/lib/swarm` directly |
| `web/src/app/network-legacy.css` | 800+ line legacy design system CSS; `network-legacy-utf8.css` is the identical UTF-8 copy. Both appear superseded by `globals.css` + Tailwind. Verify no runtime `@import` before deleting. |
| `web/src/app/network-legacy-utf8.css` | UTF-8 duplicate of the above legacy CSS. Same content, correct encoding. Still appears unused. |
| `install.sh` (root) | Duplicates `scripts/install.sh`. Root copy adds confusion. |
| `docker-compose.overflow.yml` | Exists at root level but the overflow router lives in `integrations/`. Should be consolidated or moved. |

### Dead Code Blocks

| Location | Issue |
|----------|-------|
| `desktop/rust/daemon/src/lib.rs:87` | Commented-out `use shard_gateway::rate_limiter::...` import — dead code left behind |
| `web/src/lib/api.ts:106-112` (`trySubmitDraft`) | Function body is entirely no-op (`void prompt; void workId; return false`). Either remove or implement. |
| `desktop/rust/daemon/src/lib.rs:3588` | `stability_score: 100, // TODO: calculate dynamically` — hardcoded value in production path |

### Unused/Questionable Dependencies

| Package | Location | Issue |
|---------|----------|-------|
| `localtunnel` | `web/package.json` devDeps | Development tunneling tool. Should not ship in production builds; verify it's truly dev-only. |
| `@tauri-apps/*` | `web/package.json` | Tauri desktop wrappers. These are legitimate if desktop builds are part of the release, but add 3 dependencies that 95% of deployments don't use. Consider making a separate workspace. |
| `tokio-postgres` | `desktop/rust/Cargo.toml` | Listed as workspace dependency but no PostgreSQL usage was found in the codebase. Dead dependency adding compile time. |

### Folder Restructuring Suggestions

1. **Split `lib.rs`** — The 5,986-line daemon `lib.rs` should be decomposed into: `swarm.rs` (P2P event loop), `state.rs` (SharedState and its management), `canary.rs` (rollout controller), `engine.rs` (inference engine loading), `handlers.rs` (remaining wiring). This is not cosmetic — it's the single biggest barrier to safe iteration.

2. **Move `api.rs` handlers** — The 4,100-line `api.rs` should be split by domain: `api/health.rs`, `api/chat.rs`, `api/scout.rs`, `api/ledger.rs`, `api/admin.rs`.

3. **Consolidate deploy configs** — `docker-compose.overflow.yml` at root + `deploy/demo/docker-compose.*.yml` + `deploy/testnet/docker-compose.pipeline.yml` should be referenced from a single `deploy/` README with clear instructions on which compose file to use for what.

4. **Move `integrations/` tests** — `integrations/tests/test_sla.py` is orphaned from the root test runner. Either integrate into `tests/` or add a `make test-integrations` target.

---

## Part C: Today's Action Plan (Top 5 Tasks)

### 1. Lock Down CORS on Chat Completions Proxy
**Files:** `web/src/app/api/v1/chat/completions/route.ts` (lines 108, 139)
**Action:** Replace `"Access-Control-Allow-Origin": "*"` with the value from `process.env.SHARD_CORS_ORIGINS` (already defined in `.env.example`). Use the same origin allowlist the middleware CSP already builds. This closes the open-relay abuse vector.
**Impact:** Blocks the most immediate production security risk.
**Time estimate:** ~30 minutes.

### 2. Eliminate Critical `unwrap()` Calls in Hot Paths
**Files:** `desktop/rust/crates/shard-gateway/src/rate_limiter.rs` (10 unwraps), `desktop/rust/crates/shard-common/src/common/pow_challenge.rs` (4 unwraps), `desktop/rust/crates/shard-common/src/mesh/race_router.rs` (2 unwraps)
**Action:** Replace each `unwrap()` with proper error propagation (`?`, `.unwrap_or_default()`, or `.ok_or_else(|| ...)?`). Focus on the gateway rate limiter first — it handles every incoming request.
**Impact:** Prevents panic-crashes in production under unexpected input.
**Time estimate:** ~1-2 hours.

### 3. Fix Lint Suppression and CI Gaps
**Files:** `Makefile` (line 74), `.github/workflows/ci.yml`
**Action:**
- Remove `2>/dev/null || true` from `lint-web` in the Makefile so lint failures are visible.
- Add `npm run lint` as a step in the web CI job (currently only runs `npm test`).
- Add Python test step to CI: `cd tests && python -m pytest -v`.
**Impact:** Catches regressions that currently slip through CI silently.
**Time estimate:** ~30 minutes.

### 4. Delete Dead Files and Code
**Files:** `web/src/app.tsx`, `web/src/swarm.ts`, the commented-out rate limiter import in `lib.rs:87`, the no-op `trySubmitDraft` function in `web/src/lib/api.ts`
**Action:** Delete the files, remove the dead code blocks, run tests to confirm nothing breaks. Check if any component imports from `web/src/app.tsx` or `web/src/swarm.ts` and update those imports.
**Impact:** Reduces confusion for new contributors and makes the repo leaner.
**Time estimate:** ~20 minutes.

### 5. Begin `lib.rs` Decomposition (Phase 1)
**Files:** `desktop/rust/daemon/src/lib.rs`
**Action:** Extract the `CanaryRolloutConfig`, `CanaryRolloutStatus`, `CanaryRolloutStats`, `CanaryRolloutController`, and `CanaryDecision` types + their impl blocks into a new `desktop/rust/daemon/src/canary.rs` module. This is the lowest-risk extraction because canary rollout is self-contained. Then extract `SharedState` and its construction into `state.rs`.
**Impact:** Establishes the pattern for the full decomposition. Makes code reviewable.
**Time estimate:** ~2-3 hours.

---

## Part D: The Release Roadmap

### Phase 1: Core Stabilization (Week 1-2)

**Goal:** Eliminate crash vectors, close security holes, establish CI confidence.

- [ ] Lock down CORS origins on all API proxy routes (chat, scout, PoW, telemetry)
- [ ] Replace all 42 `unwrap()` calls with proper error handling across Rust crates
- [ ] Remove dead files and dead code (see Cleanup Hitlist)
- [ ] Fix Makefile lint suppression; add Python tests and web lint to CI
- [ ] Remove `tokio-postgres` dependency if unused
- [ ] Add input validation/sanitization on all user-facing API routes (prompt length, message array bounds, model ID whitelist)
- [ ] Fix the `stability_score: 100` TODO — implement dynamic calculation or remove the field
- [ ] Verify the `network-legacy*.css` files are truly unused and delete them
- [ ] Write unit tests for the gateway rate limiter, PoW challenge, and race router crates

### Phase 2: Architecture & Performance (Week 3-4)

**Goal:** Make the codebase maintainable and address the scout latency blocker.

- [ ] Decompose `lib.rs` into 5-6 focused modules (swarm, state, canary, engine, handlers, wiring)
- [ ] Decompose `api.rs` into domain-specific route modules
- [ ] Profile and fix scout draft path latency — the 2-3x overhead vs verifier-only is the biggest product blocker
- [ ] Add connection pooling and backpressure controls for scout WebSocket connections
- [ ] Implement proper request timeout and cancellation in the daemon (currently relies on client-side timeouts)
- [ ] Add integration tests: spawn daemon + web + send chat request end-to-end
- [ ] Add load test that validates SLA under sustained traffic (p95 < 3s, error rate < 1%)
- [ ] Review and document all environment variables (currently scattered across `.env.example`, daemon CLI, web config, overflow router)

### Phase 3: Security & Hardening (Week 5-6)

**Goal:** Pass a security review and harden for adversarial environments.

- [ ] Audit all `Access-Control-Allow-Origin` headers across the codebase
- [ ] Implement API key requirement for production deployments (currently `SHARD_REQUIRE_API_KEY=false`)
- [ ] Add rate limiting on the Next.js proxy layer (currently only the Rust daemon rate-limits)
- [ ] Review the PoW difficulty settings for production — ensure they're high enough to deter abuse but low enough for legitimate browsers
- [ ] Audit the Sybil detection thresholds with real network data
- [ ] Add HTTPS enforcement for all inter-node communication (currently supports plain TCP)
- [ ] Review the `unsafe-eval` and `unsafe-inline` in CSP — minimize attack surface
- [ ] Add dependency vulnerability scanning to CI (e.g., `cargo audit`, `npm audit`)
- [ ] Harden Docker image: run as non-root, drop capabilities, read-only filesystem where possible
- [ ] Code-sign all release binaries (Windows signing is conditional on secrets; macOS/Linux have no signing)

### Phase 4: Testing & Observability (Week 7-8)

**Goal:** Achieve confidence in release quality through comprehensive testing and monitoring.

- [ ] Achieve 70%+ test coverage on Rust crates (unit tests for every public function)
- [ ] Add E2E tests for web UI: Scout join flow, Chat panel, Network status display
- [ ] Add E2E tests for Python SDK against a live daemon
- [ ] Add chaos testing: kill nodes during inference, inject network partitions, simulate slow scouts
- [ ] Verify all Prometheus metrics are correctly exported and Grafana dashboards work
- [ ] Add alerting rules for production: error rate > 5%, p95 > 5s, node count drop > 50%
- [ ] Document runbook for common production incidents (node crash, scout flood, circuit breaker trip)
- [ ] Run the full benchmark matrix on release hardware and publish results

### Phase 5: Polish & Launch (Week 9-10)

**Goal:** Final polish, documentation, and public release.

- [ ] Accessibility audit on web UI: keyboard navigation, screen reader compatibility, color contrast ratios
- [ ] Add loading skeletons and proper error states for all frontend data-fetching paths
- [ ] Finalize the "End-to-end encrypted" claim in ChatPanel footer — verify it's actually true or remove
- [ ] Update all hardcoded version strings (currently "0.6.5" appears in page.tsx, package.json, Cargo.toml, README)
- [ ] Run the full release checklist from `docs/release-rc-checklist.md`
- [ ] Execute the RC runbook from `docs/release-rc-runbook.md`
- [ ] Verify all installer packages (Linux .deb, macOS .dmg, Windows .exe, Homebrew, winget)
- [ ] Tag v1.0.0, run release workflow, verify all artifacts
- [ ] Update documentation with production deployment guide
- [ ] Announce release

---

## Appendix: File Inventory Summary

| Component | Files | Lines (approx) | Test Files | Test Coverage |
|-----------|-------|----------------|------------|---------------|
| Rust Daemon | ~30 source files | ~12,000+ | Unknown (cargo test exists) | Low-Medium |
| Rust Crates (8) | ~25 source files | ~5,000+ | Unknown | Low |
| Rust GUI | 6 source files | ~1,500 | 0 | None |
| Web Frontend | ~50 source files | ~5,000+ | 11 test files | Low-Medium |
| Python SDK | ~15 source files | ~1,200 | 6 test files | Medium |
| Root Python Tests | 3 test files | ~250 | N/A | N/A |
| Integrations | 2 source files + 1 test | ~550 | 1 test file | Low |
| Benchmarks | ~10 files | ~2,000 | 0 | None |
| Deploy/Infra | ~30 config files | ~2,000 | 0 | None |
| Scripts | ~15 files | ~2,000 | 0 | None |
| Documentation | ~20 files | ~3,000 | N/A | N/A |
| cpp/llama.cpp | Vendored (upstream) | ~100,000+ | Upstream tests | N/A |

---

*This audit was generated from a full codebase analysis on March 5, 2026 against commit on branch `claude/audit-production-readiness-OfLw7`.*

