> [!WARNING]
> **DEPRECATED**: This checklist was a point-in-time roadmap from March 5, 2026. Prioritized items have been fixed or rendered obsolete. Retained for historical context only.

# Network Execution Checklist

Date: March 5, 2026
Scope: stabilize the verifier mesh, make scouts legible and conditional, and ship production-ready operator flows.

## Rules

- Do not treat scout-assisted inference as the default path until benchmarks prove it is net-positive.
- Do not publish performance claims without raw artifacts in `reports/` or `benchmarks/results/`.
- Do not run local and EC2 verifiers on different runtime profiles during release testing.
- Every completed step must leave behind code, docs, and operator evidence.

## P0: Verifier Floor

### Step P0.1: Canonical verifier runtime parity
Goal: local Docker and EC2 verifiers must run the same effective runtime profile.

Work:
- Use one canonical runtime profile for both local and EC2 verifiers.
- Keep `deploy/release/rc1.env` as the frozen baseline.
- Layer `deploy/release/benchmark.env` on top only for matrix runs.
- Remove stale EC2 systemd overrides that silently change scout timeout, debug logging, or model settings.
- Ensure local Docker mesh loads the same env files as EC2.

Exit criteria:
- Local verifier `/health` is healthy.
- EC2 verifier `/health` is healthy.
- Local and EC2 `/v1/system/scout-config` report the same effective config for benchmark-sensitive fields.
- Local and EC2 `/health` report the same model family and runtime version.

Evidence:
- Updated deployment docs.
- Saved health/scout-config payloads for both nodes.
- Commit message references runtime parity.

### Step P0.2: Always-on verifier floor
Goal: the network must continue to issue work even if one verifier is unavailable.

Work:
- Run at least 3 controlled verifier nodes with auto-restart and health checks.
- Publish one canonical bootstrap set for operators.
- Ensure each verifier exposes a stable public address or relay path.
- Verify the web proxy has at least 2 healthy backend candidates at all times.

Exit criteria:
- Scouts polling `/v1/scout/work` do not sit idle solely because the sole verifier disappeared.
- Web proxy backend failover does not degrade into a single dead endpoint.
- Mesh keeps at least 2 healthy verifier candidates available during normal operation.

Evidence:
- Topology payload showing multiple healthy verifier nodes.
- Proxy failover proof under one-node outage.
- Updated operator runbook.

## P1: Mesh-First Verifier Behavior

### Step P1.1: Mesh forwarding as the default overflow path
Goal: any verifier should use the healthiest peer set automatically instead of acting like an isolated node.

Work:
- Keep `SHARD_MESH_FORWARD_ENABLED` on by default.
- Verify load-aware scoring uses latency plus queue depth.
- Document the env vars controlling forwarding thresholds.
- Surface forwarded-request behavior in logs and metrics.

Exit criteria:
- Local queue pressure causes requests to forward to healthier peers.
- Forwarding falls back locally on retryable remote failure without hanging the request.
- Operators can see forwarding activity in metrics or structured logs.

Evidence:
- Bench or synthetic request proof showing remote forward use.
- Metrics/log output captured in run artifacts.

### Step P1.2: Connection stability
Goal: libp2p churn must stop masking real performance.

Work:
- Reduce stale bootstrap candidates and duplicate overrides.
- Verify reconnect logic with the current browser libp2p/WebSocket paths.
- Audit relay-only paths versus direct WebSocket and QUIC paths.
- Track transport success and failure rates in release artifacts.

Exit criteria:
- Verifiers maintain stable peer connectivity during a benchmark run.
- Browser scouts no longer flap between connected and disconnected states due to known reconnect bugs.
- Transport failure counters stop dominating the health picture.

Evidence:
- Health summary before and after changes.
- Console or server logs showing stable peer sessions.

## P2: Scout Admission and UX

### Step P2.1: Conditional scout acceleration
Goal: scouts help only when they improve throughput or latency.

Work:
- Gate speculative mode on acceptance rate, active scout count, queue depth, and tail latency.
- Enforce stronger blackout and backpressure when verifier queues grow.
- Lower draft width or other speculative knobs until acceptance improves.
- Emit explicit fallback reasons when scouts are bypassed.

Exit criteria:
- Small-scale runs no longer show scouts blindly enabled when they are a net negative.
- Operators can see why scouts were allowed, throttled, or bypassed.
- Benchmark artifacts include acceptance and rejection evidence.

Evidence:
- Updated benchmark matrix.
- Summary of fallback reasons and acceptance counters.

### Step P2.2: Scout onboarding legibility
Goal: a new user should know whether they are downloading, eligible, waiting, or contributing.

Work:
- Add visible download progress and runtime eligibility states.
- Make `Join` actually start contribution where eligible.
- Show `waiting for verifier work` explicitly.
- Promote direct `shard-gui` downloads from the website.

Exit criteria:
- A new scout can tell what the browser is doing without opening devtools.
- The landing page no longer implies contribution if the worker loop is idle.

Evidence:
- Updated website copy and screenshots.
- Manual validation from a clean browser profile.

## P3: Release Readiness

### Step P3.1: Truthful metrics and docs
Goal: website and README must reflect current network reality, not optimistic estimates.

Work:
- Keep active verifier and scout counts tied to real telemetry.
- Remove stale benchmark claims and replace them only after new matrix runs.
- Add acceptance rate and scout fallback reason to public-facing summaries where appropriate.

Exit criteria:
- Website and README match the latest validated matrix.
- Operators can reconcile web telemetry with daemon health endpoints.

Evidence:
- Updated README and website snapshot.
- Linked raw benchmark artifacts.

### Step P3.2: Go/no-go gate
Goal: release only when the mesh and scout path meet explicit thresholds.

Work:
- Run the RC matrix with repeated one-node and two-node scenarios.
- Require raw artifact generation for every scenario.
- Decide `GO` or `NO-GO` from measured medians, not anecdotes.

Exit criteria:
- `docs/release-rc-checklist.md` is fully satisfiable from collected evidence.
- Release gates pass, or the project remains intentionally pre-release.

Evidence:
- `go-no-go-summary.json`
- `go-no-go-report.md`
- Per-scenario raw run JSON files

## Current Execution Order

1. Finish P0.1 runtime parity and docs.
2. Finish P1.1 mesh-forward evidence and operator visibility.
3. Re-run matrix on the stable verifier floor.
4. Use that data to decide whether P2 scout acceleration stays default-off or can be conditionally enabled.
