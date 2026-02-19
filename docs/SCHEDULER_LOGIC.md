# Scheduler Logic

Shard uses weighted node selection for next-layer routing and speculative execution targets.

## Inputs

Each candidate node is scored from these factors:

- `load` (lower is better)
- `latency_ms` (lower is better)
- `reliability_score` (success / (success + failure))
- `hardware_capability_score`
- `identity_reputation_score` (identity-bound, Ed25519 public key keyed)

## Weighted Score

The scheduler computes:

`weight = 0.30*load_factor + 0.20*latency_factor + 0.25*reliability + 0.15*hardware + 0.10*identity`

Where:

- `load_factor = clamp(1-load, 0..1)`
- `latency_factor = clamp(1/(1+latency_ms/200), 0..1)`

Candidates are sorted by descending `weight` with deterministic tie-break on node id.

## Reputation Persistence

- Reputation is keyed by node public key.
- Stored at `${DATA_DIR}/node_reputation.json`.
- Updated on signed request success/failure and persisted after mutation.

This ensures identity-bound history survives restarts and cannot be transferred via transient peer id changes.
