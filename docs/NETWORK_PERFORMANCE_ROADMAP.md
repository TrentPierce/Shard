# Network Performance Roadmap

## Goal

Ship the fastest real user path, not the most elaborate distributed demo.

That means:

- easy prompts should complete in the browser
- heavy prompts should escalate to a desktop verifier
- speculative decoding should stay inside the verifier boundary by default
- WAN browser scouts should remain an experimental research track until they beat the simpler path honestly

## Current Reality (March 11, 2026)

- Discovery, routing, and verifier controls are materially better than before.
- The compatible Llama experimental WAN path is correct and repeatable.
- The latest same-machine live-site `10 vs 10` comparison is still slower overall than verifier-only baseline:
  - baseline: `11295.1 ms` average, `11297 ms` median
  - experimental WAN: `12004.4 ms` average, `11888 ms` median
- Experimental WAN correctness was strong in that run:
  - `10/10` wait hits
  - `10/10` verification attempts
  - `4/4` accepted draft tokens on every distributed run
- Browser timing now shows where the time goes:
  - first request: `prefill_ms=258`, `decode_ms=119`, `submit_ms=9`
  - repeated identical prompt: `generate_ms=0`, `reuse=exact_prompt_cache`

The architectural conclusion is clear: WAN scout speculation is a useful experiment, but it is not the product fast path.

## Principles

1. Local browser answers should absorb the easy majority of prompts.
2. Browser-owned history and compaction should shrink network work before escalation.
3. Desktop-local speculation is a better latency bet than WAN token-level coordination.
4. Mesh routing should optimize verifier selection for non-speculative heavy work.
5. Experimental WAN work should be benchmarked separately from product claims.

## Phase 1: Router Quality

### Objective

Make the browser router a reliable gate between easy local work and heavy network work.

### Changes

- Improve heuristic routing quality for code, reasoning, and long-context prompts.
- Track `local_answer`, `network_route`, and `network_route_with_compaction` outcomes.
- Measure browser-local hit rate and escalation quality.

### Success Criteria

- Local-answer hit rate is high on simple prompt classes.
- Auto mode avoids obvious local failures on long or complex prompts.
- Browser-first routing improves user-perceived latency without increasing error rate.

## Phase 2: Browser Context Compaction

### Objective

Keep the browser as the source of truth for conversation state while reducing the prompt footprint sent to the verifier.

### Changes

- Improve summary quality for long conversations.
- Tune message-count and character-budget thresholds.
- Add metrics for compaction ratio and compaction-trigger frequency.

### Success Criteria

- Long chats remain coherent after compaction.
- Network prompt size drops materially on long sessions.
- The verifier does not need to own long-lived user session state.

## Phase 3: Desktop-Local Speculative Throughput

### Objective

Make the verifier-local speculative path beat standard generation on real hardware.

### Changes

- Continue moving speculative acceleration into the daemon boundary.
- Profile token throughput and verifier-side overhead on representative GPUs.
- Keep the public API stable while improving the local speculative backend.

### Success Criteria

- `local_speculative` beats `standard` at p50 and p95 on target machines.
- Gains hold for repeated runs, not just warm caches.
- No correctness regressions or garbled-output failures.

## Phase 4: Heavy-Work Mesh Routing

### Objective

Route non-speculative and verifier-heavy work to the best healthy nodes.

### Changes

- Prefer fast healthy verifier tiers for short standard requests.
- Keep slower nodes for spillover and longer jobs.
- Reduce reconnect churn and dial noise in the active peer set.

### Success Criteria

- Mixed-pool p95 improves for standard routed requests.
- Stable peers are preferred over noisy peers.
- Routing quality improves without starving healthy slower capacity.

## Phase 5: Experimental Research Tracks

### Objective

Keep high-risk distributed ideas alive without letting them define the shipping product.

### Tracks

- experimental WAN browser scouts
- richer browser prompt-state reuse
- regional pipeline swarms
- activation-transfer research for future multi-node inference

### Rule

These tracks do not become product claims until they beat the simpler local-first baseline in repeated measured runs.

## Benchmarking Strategy

Keep benchmark classes separate:

- browser local-answer latency
- verifier `standard` baseline
- verifier `local_speculative` uplift
- experimental WAN scout correctness
- experimental WAN wall-clock comparison
- mesh routing efficiency for standard requests

## Immediate Implementation Order

1. Router quality and observability
2. Browser compaction tuning
3. Desktop-local speculative profiling
4. Heavy-work mesh routing
5. Experimental research tracks
