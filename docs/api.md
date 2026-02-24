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

## `POST /v1/scout/draft`
- Required: `work_id`, `scout_id`, `draft_text`
- Contract:
  - `draft_tokens` may be supplied directly, or
  - server tokenizes `draft_text` and must produce non-empty tokens
- Runtime gate: `scout_id` must be PoW-verified

## Signed Envelope Endpoints
- `POST /v1/signed/register`
- `POST /v1/signed/heartbeat`
- `POST /v1/signed/metrics`
- `POST /v1/signed/deregister`

All signed endpoints require:
- valid signature
- signer-payload identity match
- strictly increasing nonce (replay protection)

