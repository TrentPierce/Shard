# Network Performance Roadmap

## Goal

Improve end-user latency and throughput by making the network selective about:

- which verifiers receive which work
- when browser scouts are worth using
- which peers remain in the active routing set

The network is no longer blocked on discovery. The main constraint is heterogeneous node performance.

## Current Reality

- Fast Fly-class verifiers stay neutral when browser scouts are attached and back off correctly.
- Slower nodes can engage browser scouts successfully, but uplift is not yet consistent enough to claim broadly.
- Mixed pools are still slowed down by treating slow and fast nodes as equivalent routing targets.
- Peer churn still creates reconnect noise and unnecessary dial attempts.

## Principles

1. Route short work to the fastest healthy verifiers first.
2. Only engage browser scouts where expected savings are positive.
3. Prune low-value peers faster than stable peers.
4. Keep benchmark classes separate so we optimize the right behavior.

## Phase 1: Tiered Verifier Routing

### Objective

Prefer the fast healthy verifier tier for short or latency-sensitive requests.

### Changes

- Add fast-tier selection to mesh forwarding.
- Restrict short requests to remote peers that are close to the best observed latency/score.
- Keep slower peers available for spillover only when:
  - the local queue is under pressure, or
  - the request is large enough that broader capacity matters more than tail latency.

### Success Criteria

- Mixed-pool short-request p95 decreases.
- Request distribution favors the fast tier without fully starving healthy slower nodes.
- No regression in correctness or local fallback behavior.

## Phase 2: Profit-Based Scout Scheduling

### Objective

Only issue scout work when expected verifier savings exceed expected wait cost.

### Changes

- Replace coarse bypass logic with profitability-aware dispatch.
- Seed adaptive wait EWMAs with pessimistic priors so the first request does not pay a blind full wait.
- Use real expected token counts instead of a hardcoded constant in saved-time estimates.

### Success Criteria

- Fast verifiers skip speculative work automatically.
- Slow verifiers preserve real speculative hits.
- Scout-attached runs do not regress p95 on fast nodes.

## Phase 3: Peer Hygiene and Stable Routing

### Objective

Reduce time spent reconnecting to low-value or dead peers.

### Changes

- Increase penalties for repeated outbound dial failures.
- Decay/remove stale peers from the active reconnect set sooner.
- Prefer stable peers with repeated successful heartbeats and low latency.

### Success Criteria

- Fewer reconnect warnings in steady state.
- Lower background dial noise on Fly and local nodes.
- Faster convergence after restart.

## Phase 4: Slow-Node Verifier Optimization

### Objective

Improve the verifier baseline on slower hardware before asking scouts to accelerate it.

### Changes

- Maintain separate short and long runtime profiles.
- Tune long-request queue caps and wait budgets per node class.
- Reassess model/runtime settings for slower verifier tiers.

### Success Criteria

- Slow-node verifier-only p95 improves materially.
- Slow-node speculative runs have a realistic chance to outperform baseline.

## Benchmarking Strategy

Use separate benchmark classes and do not blend their claims:

- `short_rc_stability`
- `fast-node verifier baseline`
- `slow-node speculative uplift`
- `mixed-mesh routing efficiency`

## Deferred Work

- Add streaming mesh-forward proxy support so forwarded `/v1/chat/completions` requests can preserve SSE chunking instead of falling back to non-stream-only forwarding.

## Immediate Implementation Order

1. Tiered verifier routing
2. Profit-based scout scheduling improvements
3. Peer hygiene
4. Slow-node verifier optimization
