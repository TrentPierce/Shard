# Security Model

## Authentication Policy Matrix
- `SHARD_REQUIRE_API_KEY=false`:
  - public chat allowed without key
  - private route (`X-Shard-Route: private`) requires key
- `SHARD_REQUIRE_API_KEY=true`:
  - all chat completions require key
- invalid API key:
  - request rejected with `401`

## PoW and Sybil Resistance
- Scout work and draft ingress are protected by PoW verification.
- Flow:
  1. `GET /v1/pow/challenge`
  2. solve challenge
  3. `POST /v1/pow/verify`
  4. use verified `scout_id` on work/draft routes

## Replay and Signature Protection
- Signed envelope routes validate:
  - signature authenticity
  - signer identity binding
  - monotonic nonce

## Admin Controls
- `POST /admin/api-keys` requires `X-Shard-Admin` matching `SHARD_ADMIN_KEY`.

