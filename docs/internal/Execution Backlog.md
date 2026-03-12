Execution Backlog (Local-First Product Program)

## 1. Product Rule

Shard now ships around one execution rule:

1. Answer easy prompts in the browser when possible.
2. Escalate harder prompts to a desktop verifier.
3. Keep speculative acceleration inside the verifier boundary by default.
4. Treat WAN browser scouts as benchmark-only research unless they beat the simpler path honestly.

This backlog replaces the old scout-first product framing. Experimental WAN work still exists, but it is no longer the main product roadmap.

## 2. Active Epics

1. E1 Browser Router Quality and Product UX
2. E2 Browser-Owned Context Compaction
3. E3 Desktop-Local Speculative Throughput
4. E4 Heavy-Work Mesh Routing and Failover
5. E5 Release, SDK, and Workflow Reliability
6. E6 Production Readiness and Operations
7. E7 Experimental Research Tracks

## 3. Priority Queue

### E1 Browser Router Quality and Product UX

1. P0-T1 Add browser-router outcome telemetry (`local_answer`, `network_route`, `network_route_with_compaction`, fallback reason).

- Owner: Web Lead
- Complexity: M
- Acceptance: Route outcomes are observable in browser telemetry and regression tests cover route choice for simple, code, long-context, and failure-fallback prompts.

2. P0-T2 Tune `Auto` routing heuristics for code, reasoning, and long-context prompts.

- Owner: Web Lead
- Complexity: M
- Acceptance: Misroutes drop on the maintained prompt suite and Auto mode keeps obvious easy prompts local.

3. P1-T3 Improve user-visible route status and failure messaging.

- Owner: Web Lead + Design
- Complexity: S
- Acceptance: Users can tell whether the request stayed local, escalated, or fell back.

### E2 Browser-Owned Context Compaction

1. P0-T4 Add compaction metrics and trigger visibility.

- Owner: Web Lead
- Complexity: M
- Acceptance: Browser telemetry captures compaction ratio, compaction count, and when compaction was chosen before network escalation.

2. P1-T5 Improve summary quality for long chats.

- Owner: Web Lead + ML Infra
- Complexity: M
- Acceptance: Long sessions remain coherent after compaction in the maintained scenario set.

3. P1-T6 Add regression tests for compacted escalation payloads.

- Owner: Web Lead + QA
- Complexity: S
- Acceptance: Browser-to-daemon payload tests prove compacted history stays well-formed and deterministic.

### E3 Desktop-Local Speculative Throughput

1. P0-T7 Benchmark `local_speculative` against `standard` on target hardware classes.

- Owner: Backend Lead
- Complexity: M
- Acceptance: Controlled repeated runs report p50, p95, throughput, and correctness for both modes.

2. P0-T8 Remove avoidable daemon overhead in the local speculative path.

- Owner: Backend Lead
- Complexity: L
- Acceptance: Local speculative mode beats or matches standard on the primary target machine class without correctness regressions.

3. P1-T9 Move toward a true verifier-local draft+target path where supported.

- Owner: Backend Lead + ML Infra
- Complexity: L
- Acceptance: The daemon can run a meaningful local draft+verify path without changing the public API.

### E4 Heavy-Work Mesh Routing and Failover

1. P0-T10 Improve verifier selection for standard and local speculative network routes.

- Owner: Backend Lead + Networking Lead
- Complexity: M
- Acceptance: Short requests prefer healthy fast tiers and failover remains predictable under one-node degradation.

2. P1-T11 Reduce reconnect churn and dial noise in mixed verifier pools.

- Owner: Networking Lead
- Complexity: M
- Acceptance: Stable peers are preferred and noisy peers stop dominating the active pool.

3. P1-T12 Add mixed-pool routing benchmarks for heavy work.

- Owner: Backend Lead + QA
- Complexity: M
- Acceptance: Benchmark artifacts compare one-node, multi-node, and failover scenarios for the product path.

### E5 Release, SDK, and Workflow Reliability

1. P0-T13 Keep root `VERSION` as the single source of truth across web, Rust, SDKs, docs, and installers.

- Owner: Release Engineer
- Complexity: S
- Acceptance: CI fails on mismatch and release automation updates the Python SDK, web app, and desktop artifacts consistently.

2. P0-T14 Keep GitHub Actions and release lanes healthy.

- Owner: Release Engineer
- Complexity: S
- Acceptance: CI is green, release workflows publish release artifacts, and optional SDK publish lanes fail loudly but do not silently drift.

3. P1-T15 Standardize release notes and published benchmark claims.

- Owner: Release Engineer + Tech Writer
- Complexity: S
- Acceptance: Product release notes refer to the local-first path and separate experimental WAN data from product claims.

### E6 Production Readiness and Operations

1. P0-T16 Define ship gates for browser-local latency, verifier-routed latency, error rate, and failover behavior.

- Owner: SRE Lead
- Complexity: S
- Acceptance: Release RC docs and dashboards reflect the product path rather than scout participation.

2. P0-T17 Validate product-mode soak tests before public release.

- Owner: SRE + QA
- Complexity: M
- Acceptance: Browser-local, network-only, and auto-routed sessions pass the soak window without false-ready states or sustained failures.

3. P1-T18 Tighten alerting around route failures and false healthy states.

- Owner: SRE
- Complexity: M
- Acceptance: Operators can distinguish browser-runtime failures, verifier degradation, and proxy failover churn quickly.

### E7 Experimental Research Tracks

1. P1-T19 Keep experimental WAN scouts benchmarkable but out of the main product startup path.

- Owner: Web Lead + Backend Lead
- Complexity: S
- Acceptance: Experimental WAN remains opt-in and public claims use separate benchmark artifacts.

2. P2-T20 Explore richer browser prompt-state reuse.

- Owner: Web Lead
- Complexity: M
- Acceptance: Any reuse optimization has strict correctness guards and does not change the default product contract.

3. P2-T21 Evaluate regional pipeline or activation-transfer research only after the product path is solid.

- Owner: ML Infra + Architect
- Complexity: L
- Acceptance: Research notes exist, but no product commitment is made without repeated measured wins.

## 4. Current Sequencing

1. Phase A: release/sdk hygiene plus docs and backlog realignment
2. Phase B: router telemetry and Auto routing quality
3. Phase C: browser compaction quality and regression coverage
4. Phase D: desktop-local speculative throughput proof
5. Phase E: heavy-work mesh routing and failover tuning
6. Phase F: experimental research tracks

## 5. Production Gate

Shard is ready for a normal production launch only when all of the following are true:

1. Browser `Auto` routing is measurably sane on the maintained prompt suite.
2. Browser-local answers are fast and correct on the supported device/browser set.
3. Verifier `local_speculative` is at least as good as `standard` on the target hardware class, with no correctness regressions.
4. Multi-backend failover and mesh routing behave predictably under node degradation.
5. Release, SDK publish, and version parity workflows are green.
6. SDK and contributor-control endpoints let developers both consume and contribute capacity programmatically.
7. Experimental WAN data is documented separately and is not required for the ship decision.

## 6. Current Notes (2026-03-12)

- The browser local-first router is the shipped product path.
- Browser-owned conversation compaction exists and needs better observability and tuning.
- Experimental WAN correctness is proven on the compatible Llama pair, but the path is still slower than the verifier-only baseline in controlled comparison.
- The next major product-performance milestone is verifier-local speculative uplift over `standard`, not more WAN draft coordination.
- The Python SDK now exposes signed contributor-control endpoints so developers can register and report a verifier node programmatically.
