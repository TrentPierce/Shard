# Shard Investor Audit (2026-02-13)

## Executive Decision

**I would invest: $2.5M pre-seed via a tranched SAFE**.

- **Initial close:** $1.0M immediately for 6 months runway.
- **Milestone tranche A ($750k):** released when reproducible builds and CI test gates are green on Python, Rust, and web.
- **Milestone tranche B ($750k):** released when real end-to-end inference verification replaces scaffolding and production security controls are validated.

**Valuation posture:** founder-friendly if milestones are accepted as objective release gates.

## Why I Would Invest

1. **The architecture is differentiated and technically coherent.**
   - Shard combines local full-model verification and browser scout draft generation, with a documented P2P split across Python, Rust, and web layers.
2. **The repo already has meaningful system boundaries and docs.**
   - API, sidecar networking, and client are separated and described with an implementation roadmap.
3. **The upside is large if execution risk is reduced.**
   - If Shard ships a reliable decentralized inference path with measurable latency/cost wins, it can attract both OSS developer mindshare and enterprise edge-inference interest.

## Material Risks That Reduce Check Size

1. **Test and artifact hygiene are not yet investor-grade.**
   - Running `pytest -q` currently fails during collection due to platform-specific tests and missing binary artifacts.
2. **Frontend reproducibility is broken.**
   - `npm ci` fails because lockfile and manifest are out of sync.
3. **Core claims are still partially scaffolded.**
   - Existing project docs still identify hardcoded/scaffolded inference and verification as blockers to production readiness.

## Full-Stack Product/Engineering Plan I Would Require

### 0–30 days (must-do)

- **Stabilize CI and deterministic installs**
  - Make `npm ci` pass from clean checkout.
  - Scope Python tests so repo-wide collection does not execute local/manual artifact tests by default.
  - Add CI matrix for Linux/macOS (and Windows where relevant for DLL paths).
- **Define acceptance benchmarks**
  - P50/P95 latency, tokens/sec, success rate, and cost-per-1k tokens vs a centralized baseline.
- **Security baseline**
  - Enforce API key auth in non-dev environments, tighten CORS defaults, and add structured audit logging for auth/rate-limit events.

### 31–90 days (proof of scale)

- **Replace remaining scaffolded inference paths** with real BitNet-backed generation + deterministic verification.
- **Ship reliability primitives**
  - Peer reputation persistence and ban/unban governance workflows.
  - Failure injection tests for partition, reconnect, and control-plane degradation.
- **Commercial readiness**
  - Hosted bootstrap service SLA, metrics dashboard, and clear contributor economics.

### 90–180 days (go-to-market readiness)

- **Enterprise controls**: tenancy, quotas, observability export, policy controls.
- **Developer flywheel**: SDK polishing, sample apps, self-serve deployment templates.
- **Ecosystem moat**: verified scout marketplace, model/plugin compatibility certification.

## Investment Milestones and Governance

- **Monthly board-style review** with a shared KPI scorecard.
- **Capital release tied to measurable outcomes**, not narrative progress.
- **Hiring priorities:**
  1. Staff-level distributed systems engineer (Rust/libp2p).
  2. Senior inference/runtime engineer (BitNet + verification loop).
  3. Product-minded full-stack engineer (web reliability + DX).
  4. Security engineer (abuse resistance + auth + threat modeling).

## KPI Targets I’d Track

- **Reliability:** >99.5% successful completion rate on supported workloads.
- **Performance:** demonstrable P95 latency improvement over single-node local baseline.
- **Network health:** active peers, useful contribution ratio, median reconnect time.
- **Economics:** marginal cost per generated token and contributor retention.
- **Trust:** verified draft acceptance rate, fraud/bad-scout incidence trend.

## Bottom Line

Shard is investable **now**, but only with disciplined milestone-based financing. The technical thesis is credible and potentially category-defining; execution maturity (reproducibility, verification integrity, and production hardening) is the key determinant for increasing investment in the next round.
