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
5. E5 Low-Power Browser Capability and WebNN Prep
6. E6 Release, SDK, and Workflow Reliability
7. E7 Production Readiness and Operations
8. E8 Experimental Research Tracks

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

3. P0-T3 Add a per-prompt route visualizer to `/chat`.

- Owner: Web Lead
- Complexity: M
- Acceptance: Each prompt shows the browser decision, compaction state, transport, backend, and mesh-forward outcome directly in the chat UI.

4. P1-T4 Improve user-visible route status and failure messaging.

- Owner: Web Lead + Design
- Complexity: S
- Acceptance: Users can tell whether the request stayed local, escalated, or fell back.

### E2 Browser-Owned Context Compaction

1. P0-T5 Add compaction metrics and trigger visibility.

- Owner: Web Lead
- Complexity: M
- Acceptance: Browser telemetry captures compaction ratio, compaction count, and when compaction was chosen before network escalation.

2. P0-T6 Add semantic compaction so relevant older turns survive escalation.

- Owner: Web Lead + ML Infra
- Complexity: M
- Acceptance: Network-bound prompts keep the most relevant older messages, not just the newest window, and regression tests cover the ranking path.

3. P1-T7 Improve summary quality for long chats.

- Owner: Web Lead + ML Infra
- Complexity: M
- Acceptance: Long sessions remain coherent after compaction in the maintained scenario set.

4. P1-T8 Add regression tests for compacted escalation payloads.

- Owner: Web Lead + QA
- Complexity: S
- Acceptance: Browser-to-daemon payload tests prove compacted history stays well-formed and deterministic.

### E3 Desktop-Local Speculative Throughput

1. P0-T9 Benchmark `local_speculative` against `standard` on target hardware classes.

- Owner: Backend Lead
- Complexity: M
- Acceptance: Controlled repeated runs report p50, p95, throughput, and correctness for both modes.

2. P0-T10 Remove avoidable daemon overhead in the local speculative path.

- Owner: Backend Lead
- Complexity: L
- Acceptance: Local speculative mode beats or matches standard on the primary target machine class without correctness regressions.

3. P0-T11 Add adaptive local speculative bypass when live timings show no savings.

- Owner: Backend Lead
- Complexity: M
- Acceptance: `local_speculative` avoids self-inflicted regressions on already-fast verifiers or negative-savings windows.

4. P1-T12 Move toward a true verifier-local draft+target path where supported.

- Owner: Backend Lead + ML Infra
- Complexity: L
- Acceptance: The daemon can run a meaningful local draft+verify path without changing the public API.

### E4 Heavy-Work Mesh Routing and Failover

1. P0-T13 Improve verifier selection for standard and local speculative network routes.

- Owner: Backend Lead + Networking Lead
- Complexity: M
- Acceptance: Short requests prefer healthy fast tiers and failover remains predictable under one-node degradation.

2. P0-T14 Add stronger queue-aware and region-aware penalties to the mesh scorer.

- Owner: Backend Lead + Networking Lead
- Complexity: M
- Acceptance: Live heavy-work runs stop selecting near-saturated or cross-region verifier targets unless the improvement is real.

3. P0-T15 Reduce heavy-work mesh p95 on the Fly pool, with focused `iad` reruns.

- Owner: Backend Lead + QA
- Complexity: M
- Acceptance: The maintained live multi-node benchmark shows lower tail latency for the mesh-enabled product path in `iad`.

4. P1-T16 Reduce reconnect churn and dial noise in mixed verifier pools.

- Owner: Networking Lead
- Complexity: M
- Acceptance: Stable peers are preferred and noisy peers stop dominating the active pool.

5. P1-T17 Add mixed-pool routing benchmarks for heavy work.

- Owner: Backend Lead + QA
- Complexity: M
- Acceptance: Benchmark artifacts compare one-node, multi-node, and failover scenarios for the product path.

6. P1-T18 Feed recent forward-latency history and freshness into the mesh scorer.

- Owner: Backend Lead + Networking Lead
- Complexity: M
- Acceptance: Slow-but-alive peers are down-ranked by recent actual forward latency instead of being selected solely from live probe snapshots.

### E5 Low-Power Browser Capability and WebNN Prep

1. P1-T19 Add browser accelerator telemetry that separates current WebGPU runtime support from future WebNN eligibility.

- Owner: Web Lead
- Complexity: M
- Acceptance: Capability surfaces report `backgroundAcceleration`, `lowPowerEligible`, and probe warm-state without changing the default browser runtime path.

2. P0-T20 De-risk WebNN with embeddings and background utility work before draft-token generation.

- Owner: Web Lead + ML Infra
- Complexity: M
- Acceptance: A worker-based WebNN path is proven on a low-risk task before any speculative draft-token claim is made.

3. P2-T21 Unify browser model manifests across WebGPU and WebNN variants.

- Owner: ML Infra + Release Engineer
- Complexity: L
- Acceptance: One logical model version can declare both TVM/WebGPU and ONNX/WebNN artifacts without version drift.

### E6 Release, SDK, and Workflow Reliability

1. P0-T22 Keep root `VERSION` as the single source of truth across web, Rust, SDKs, docs, and installers.

- Owner: Release Engineer
- Complexity: S
- Acceptance: CI fails on mismatch and release automation updates the Python SDK, web app, and desktop artifacts consistently.

2. P0-T23 Keep GitHub Actions and release lanes healthy.

- Owner: Release Engineer
- Complexity: S
- Acceptance: CI is green, release workflows publish release artifacts, and optional SDK publish lanes fail loudly but do not silently drift.

3. P1-T24 Standardize release notes and published benchmark claims.

- Owner: Release Engineer + Tech Writer
- Complexity: S
- Acceptance: Product release notes refer to the local-first path and separate experimental WAN data from product claims.

### E7 Production Readiness and Operations

1. P0-T25 Define ship gates for browser-local latency, verifier-routed latency, error rate, and failover behavior.

- Owner: SRE Lead
- Complexity: S
- Acceptance: Release RC docs and dashboards reflect the product path rather than scout participation.

2. P0-T26 Validate product-mode soak tests before public release.

- Owner: SRE + QA
- Complexity: M
- Acceptance: Browser-local, network-only, and auto-routed sessions pass the soak window without false-ready states or sustained failures.

3. P1-T27 Tighten alerting around route failures and false healthy states.

- Owner: SRE
- Complexity: M
- Acceptance: Operators can distinguish browser-runtime failures, verifier degradation, and proxy failover churn quickly.

### E8 Experimental Research Tracks

1. P1-T28 Keep experimental WAN scouts benchmarkable but out of the main product startup path.

- Owner: Web Lead + Backend Lead
- Complexity: S
- Acceptance: Experimental WAN remains opt-in and public claims use separate benchmark artifacts.

2. P2-T29 Explore richer browser prompt-state reuse.

- Owner: Web Lead
- Complexity: M
- Acceptance: Any reuse optimization has strict correctness guards and does not change the default product contract.

3. P2-T30 Evaluate regional pipeline or activation-transfer research only after the product path is solid.

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
- Mesh routing now has endpoint-history scoring, but tail-latency tuning still needs more measured work on live multi-node pools.
- Browser capability surfaces now distinguish current WebGPU execution from future low-power WebNN eligibility, but the ONNX/WebNN runtime path does not ship yet.
- The next concrete product-path wins are: better route visualization, better easy-prompt local retention, stronger mesh selection under overload, and a `local_speculative` path that stops opting into non-productive work.
