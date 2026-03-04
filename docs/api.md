# API Contracts

## Canonical Schemas
- `docs/schemas/v1.chat-completions.request.schema.json`
- `docs/schemas/v1.scout-work.response.schema.json`
- `docs/schemas/v1.scout-draft.request.schema.json`
- `docs/schemas/signed-envelope.schema.json`

## `POST /v1/chat/completions`
- Header `X-Shard-Inference-Mode`:
  - `standard` => verifier-only generation
  - `distributed` => speculative flow enabled
  - `speculative` => speculative flow enabled
- Header `Authorization: Bearer <api_key>`:
  - required when `SHARD_REQUIRE_API_KEY=true`
  - always required when `X-Shard-Route: private`

## `GET /v1/scout/work`
- Required query: `scout_id`
- Runtime gate: `scout_id` must be PoW-verified (`/v1/pow/challenge`, `/v1/pow/verify`)
- Overload and admission responses:
  - `429` with `detail: "scout_backpressure"` when soft admission/backpressure is active.
  - `503` with `detail: "scout_circuit_open"` when hard circuit mode is active.
  - `429` with `detail: "scout_poll_rate_limited"` for per-scout poll pacing.
  - `429` with `detail: "scout_active_cap_reached"` when verifier active-scout cap is reached.
  - All transient rejections include `retry_after_ms`.

## `POST /v1/scout/draft`
- Required: `work_id`, `scout_id`, `draft_text`
- Contract:
  - `draft_tokens` may be supplied directly, or
  - server tokenizes `draft_text` and must produce non-empty tokens
- Runtime gate: `scout_id` must be PoW-verified
- Overload and admission responses:
  - `429` with `detail: "scout_draft_backpressure"` when soft admission/backpressure is active.
  - `503` with `detail: "scout_draft_circuit_open"` when hard circuit mode is active.
  - `429` with `detail: "scout_draft_rate_limited"` for per-scout submit pacing.
  - All transient rejections include `retry_after_ms`.
- Optional fraud-proof field: `spot_check`
  - Contains `input_a`, `weights_b`, `claimed_c`, `m`, `k`, `n`, optional `seed`
  - Verifier enforces probabilistic matmul spot-check before accepting draft
  - Runtime config:
    - `SHARD_SPOTCHECK_SAMPLE_RATE`
    - `SHARD_SPOTCHECK_TOLERANCE`
    - `SHARD_SPOTCHECK_MIN_ROWS`

## Signed Envelope Endpoints
- `POST /v1/signed/register`
- `POST /v1/signed/heartbeat`
- `POST /v1/signed/metrics`
- `POST /v1/signed/deregister`

All signed endpoints require:
- valid signature
- signer-payload identity match
- strictly increasing nonce (replay protection)

## Bootstrap Registry
- `GET /v1/system/bootstrap` now returns both:
  - `known_bootstraps` (live stability view from connected peers)
  - `registered_bootstraps` (persisted registry)
- `POST /v1/system/bootstrap` persists bootstrap candidates and survives daemon restarts.

## Scheduler Audit
- `GET /v1/system/scheduler-decisions`
  - Returns recent next-layer scheduling decisions with candidate inputs and selected peers.
  - Includes load, latency, reliability, hardware, and identity scoring inputs used for decisioning.

## Model Rollout Controls
- `GET /v1/system/model-rollout`
  - Returns canary rollout config, status, and canary performance counters.
- `POST /v1/system/model-rollout/reset-rollback`
  - Clears auto-rollback state and resets canary samples.
  - Requires `X-Shard-Admin` if `SHARD_ADMIN_KEY` is configured.

## Scout Runtime Config and Admission State
- `GET /v1/system/scout-config`
  - Returns effective scout scheduling config from environment/defaults plus live admission state.
  - Includes:
    - backpressure thresholds
    - admission thresholds
    - per-scout poll/draft rate limits
    - active-scout cap levels (`allow`, `soft_backpressure`, `hard_circuit`)
    - runtime queue/latency and current admission mode

## Compatibility-Aware Scheduling
- `/layers/next` includes `model_pair_compatible`.
- Incompatible draft/verifier pairs return an empty peer set with a compatibility detail message.
