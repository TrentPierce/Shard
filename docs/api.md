# API Contracts

## Product Note

The browser product experience is local-first. For simple prompts, the browser may answer without calling `POST /v1/chat/completions` at all.

When the browser escalates to the network, it uses the same OpenAI-compatible API surface and may send either:

- the raw conversation window
- a compacted conversation window produced in the browser

Before that escalation, the browser may run semantic ranking and compaction in a worker lane that prefers ONNX/WebNN and falls back to ONNX/WASM or hashed embeddings.

The verifier remains stateless with respect to long-lived user session ownership.

## Canonical Schemas

- `docs/schemas/v1.chat-completions.request.schema.json`
- `docs/schemas/v1.scout-work.response.schema.json`
- `docs/schemas/v1.scout-draft.request.schema.json`
- `docs/schemas/signed-envelope.schema.json`

## `POST /v1/chat/completions`

### Header `X-Shard-Inference-Mode`

- `standard`
  - verifier-only generation
  - clean baseline path
- `local_speculative`
  - verifier-local speculative path
  - explicit opt-in acceleration path until it beats `standard` on target hardware
- `experimental_wan`
  - explicit benchmark-only browser-scout path
- `distributed`
  - legacy alias for `experimental_wan`
- `speculative`
  - legacy alias for `experimental_wan`
- `auto`
  - accepted by the daemon
  - currently resolves to `standard` for network work, while browser product surfaces usually decide the route before the daemon sees the request

### Browser Router Outcomes

The browser router currently resolves one of:

- `local_answer`
- `network_route`
- `network_route_with_compaction`

These outcomes are browser-side product behavior, not extra daemon API modes.

### Browser-side semantic compaction

The browser may semantically rank older messages before producing a compacted network payload.

- preferred backend: `onnx-webnn`
- fallback backend: `onnx-wasm`
- last-resort fallback: deterministic hashed embeddings

This is browser product behavior and does not change the public daemon request schema.

### Header `Authorization: Bearer <api_key>`

- required when `SHARD_REQUIRE_API_KEY=true`
- always required when `X-Shard-Route: private`

## `POST /v1/agents/tasks`

This is the receipt-first v1 workflow surface for cross-topology agent execution.

- `workflow_kind` must be `research_brief`
- live web search is out of scope for v1
- `sources[]` are caller-provided documents or snippets
- blank source IDs are auto-filled as `source-{n}`
- duplicate source IDs or blank source content return `400`
- the orchestrating daemon is the canonical receipt writer for the workflow
- the response includes:
  - `execution` summary
  - append-only `receipts`
  - a reconstructable `provenance` graph derived from those receipts
  - optional `detail` when the workflow fails but still returns a debuggable execution bundle

### Request Body

- `workflow_kind`
  - currently only `research_brief`
- `question`
  - required research question or brief prompt
- `sources`
  - array of:
    - `id`
    - optional `title`
    - `content`
- `policy`
  - `allowed_supply_tiers`
  - `trust_tier`
  - optional `budget_limit`
  - optional `deadline_ms`
  - `capability_tags`
  - `fallback_order`
  - optional `data_residency`
    - omit it when you do not need a hard residency boundary
  - optional `max_public_spend`
- `model`
  - optional model hint

### Example

```bash
curl -X POST http://127.0.0.1:9091/v1/agents/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "workflow_kind": "research_brief",
    "question": "Summarize the strongest launch story for Shard V1.",
    "sources": [
      {
        "id": "market-notes",
        "title": "Market notes",
        "content": "Teams care about routing clarity, failure visibility, and cross-topology policy control."
      },
      {
        "id": "ops-notes",
        "title": "Operator notes",
        "content": "Operators contribute specialist capacity when it helps their own workflows first."
      }
    ],
    "policy": {
      "allowed_supply_tiers": ["personal", "private", "public"],
      "trust_tier": "verified_mesh",
      "budget_limit": 1.25,
      "deadline_ms": 45000,
      "capability_tags": ["planning", "summarization", "synthesis"],
      "fallback_order": ["personal", "private", "public"],
      "max_public_spend": 0.35
    }
  }'
```

### Response Notes

- `execution.status` is one of:
  - `running`
  - `completed`
  - `failed`
  - `orphaned`
- `receipts[].event_kind` is one of:
  - `planned`
  - `candidate_ranked`
  - `dispatched`
  - `completed`
  - `failed`
  - `fallback_applied`
  - `orphaned`
- each receipt carries `parent_receipt_id`
- failed workflow submissions still return persisted `execution`, `receipts`, and `provenance` so the unhappy path remains inspectable
- the provenance graph is reconstructed from receipt parentage rather than maintained as mutable coordinator state
- if `policy` is omitted entirely, Shard applies the product defaults used by the web demo and SDK:
  - `trust_tier = verified_mesh`
  - `budget_limit = 1.25`
  - `deadline_ms = 45000`
  - `capability_tags = [planning, summarization, synthesis]`
  - `fallback_order = [personal, private, public]`
  - `max_public_spend = 0.35`
- completed workflow artifacts include:
  - `brief`
  - `planner_notes`
  - `sub_questions[]`
  - `selected_source_ids[]`
  - `source_summaries[]`

## `GET /v1/executions/{execution_id}`

Returns the latest execution summary for a workflow.

- `execution_id`
  - returned by `POST /v1/agents/tasks`
- response body:
  - `ok`
  - `execution`

The summary includes:

- `status`
- `current_step`
- `question`
- `source_count`
- `latest_summary`
- optional final `result`

## `GET /v1/executions/{execution_id}/receipts`

Returns the append-only receipt log for one workflow execution.

- receipts are stored locally by the orchestrating daemon in SQLite-backed append-only storage
- receipt integrity is independent from payment or settlement
- if the daemon restarts with in-flight work that cannot be reconciled, recovery emits `orphaned` receipts instead of hiding the incomplete branch

Important receipt fields:

- `receipt_id`
- `execution_id`
- `step_id`
- `attempt_id`
- `parent_receipt_id`
- `policy_snapshot`
- optional `task_context`
- `candidate_rankings`
- `selected_candidate`
- `supply_tier`
- `trust_tier`
- `capability_match_reason`
- `estimated_cost_usd`
- `actual_cost_usd`
- `latency_ms`
- `failure_reason`
- `fallback_reason`
- `node_identity`
- `agent_identity`
- `model_metadata`

## `GET /v1/executions/{execution_id}/provenance`

Returns the reconstructable provenance graph for one workflow.

- graph nodes are derived directly from receipts
- graph edges are derived from `parent_receipt_id`
- incomplete or orphaned branches remain visible

Response fields:

- `execution_id`
- optional `root_receipt_id`
- `nodes[]`
- `edges[]`
- `incomplete`

The web demo for this flow lives at `/provenance`.

## `GET /v1/capabilities`

Returns the daemon-visible capability descriptors used for routing and ranking.

Capability records include:

- `candidate_id`
- `display_name`
- `supply_tier`
- `trust_tier`
- optional `capability_tier`
- optional `gpu_available`
- optional `public_api`
- optional `endpoint`
- optional `queue_depth`
- optional `latency_ms`
- optional `score`
- `tags`
- optional `role`
- optional `healthy`
- optional `selection_reason`

## Programmatic Contribution

The public developer story is now split into two responsibilities:

- consume inference through `POST /v1/chat/completions`
- contribute compute through daemon contributor/control-plane endpoints

The browser remains the local-first router for end users, but contributor registration and liveness are daemon responsibilities.

### Local Contributor Control

- `GET /node/status`
- `GET /health`
- `POST /node/toggle-participation`

These are the simplest programmatic controls for a local desktop node that is already running.

### Signed Contributor Control Plane

- `POST /signed/register-node`
- `POST /signed/heartbeat`
- `POST /signed/metrics-report`
- `POST /signed/deregister-node`

These endpoints are the canonical API-first contribution surface for desktop nodes today.

Contract rules:

- requests are wrapped in a signed envelope
- `signer_pubkey_hex` must match `payload.node_pubkey`
- nonces must strictly increase per signer
- timestamps must be fresh

Current payload families:

- registration / deregistration
  - `node_pubkey`
  - `role`
  - optional `capacity`
  - optional `timestamp_ms`
- heartbeat
  - `node_pubkey`
  - `role`
  - `queue_depth`
  - `node_latency_ms`
  - `uptime_seconds`
  - optional `capability_tier`
  - optional `gpu_available`
  - optional `accepts_scout_work`
  - optional `public_api`
  - optional `public_api_addr`
  - optional `timestamp_ms`
- metrics report
  - `node_pubkey`
  - `role`
  - `queue_depth`
  - `node_latency_ms`
  - `uptime_seconds`
  - optional `capability_tier`
  - optional `gpu_available`
  - optional `accepts_scout_work`
  - optional `public_api`
  - optional `timestamp_ms`

The Python SDK exposes these flows through `client.contribution`.

## `GET /v1/scout/work`

This is an experimental WAN endpoint, not part of the default product request path.

- Required query: `scout_id`
- Runtime gate: `scout_id` must be PoW-verified via `/v1/pow/challenge` and `/v1/pow/verify`
- Overload and admission responses:
  - `429` with `detail: "scout_backpressure"` when soft admission or backpressure is active
  - `503` with `detail: "scout_circuit_open"` when hard circuit mode is active
  - `429` with `detail: "scout_poll_rate_limited"` for per-scout poll pacing
  - `429` with `detail: "scout_active_cap_reached"` when verifier active-scout cap is reached
  - transient rejections include `retry_after_ms`

## `POST /v1/scout/draft`

This is an experimental WAN endpoint, not part of the default product request path.

- Required: `work_id`, `scout_id`, `draft_text`
- Contract:
  - `draft_tokens` may be supplied directly, or
  - server tokenizes `draft_text` and must produce non-empty tokens
- Runtime gate: `scout_id` must be PoW-verified
- Overload and admission responses:
  - `429` with `detail: "scout_draft_backpressure"` when soft admission or backpressure is active
  - `503` with `detail: "scout_draft_circuit_open"` when hard circuit mode is active
  - `429` with `detail: "scout_draft_rate_limited"` for per-scout submit pacing
  - transient rejections include `retry_after_ms`
- Optional fraud-proof field: `spot_check`
  - contains `input_a`, `weights_b`, `claimed_c`, `m`, `k`, `n`, and optional `seed`
  - verifier enforces probabilistic matmul spot-check before accepting draft
  - runtime config:
    - `SHARD_SPOTCHECK_SAMPLE_RATE`
    - `SHARD_SPOTCHECK_TOLERANCE`
    - `SHARD_SPOTCHECK_MIN_ROWS`

## Signed Envelope Endpoints

- `POST /signed/register-node`
- `POST /signed/heartbeat`
- `POST /signed/metrics-report`
- `POST /signed/deregister-node`

All signed endpoints require:

- valid signature
- signer-payload identity match
- strictly increasing nonce for replay protection

## Bootstrap Registry

- `GET /v1/system/bootstrap` returns both:
  - `known_bootstraps` for the live stability view from connected peers
  - `registered_bootstraps` for the persisted registry
- `POST /v1/system/bootstrap` persists bootstrap candidates across daemon restarts

## Scheduler Audit

- `GET /v1/system/scheduler-decisions`
  - returns recent next-layer scheduling decisions with candidate inputs and selected peers
  - includes load, latency, reliability, hardware, and identity scoring inputs

## Model Rollout Controls

- `GET /v1/system/model-rollout`
  - returns canary rollout config, status, and canary performance counters
- `POST /v1/system/model-rollout/reset-rollback`
  - clears auto-rollback state and resets canary samples
  - requires `X-Shard-Admin` if `SHARD_ADMIN_KEY` is configured

## Scout Runtime Config and Admission State

- `GET /v1/system/scout-config`
  - returns effective scout scheduling config plus live admission state
  - includes:
    - backpressure thresholds
    - admission thresholds
    - per-scout poll and draft rate limits
    - active-scout cap levels
    - runtime queue and latency state

## Compatibility-Aware Scheduling

- `/layers/next` includes `model_pair_compatible`
- incompatible draft and verifier pairs return an empty peer set with a compatibility detail message
