# Shard Architecture

## Components
- `desktop/rust/daemon`: control plane, verifier runtime, mesh networking, and ingress APIs.
- `web`: browser scout UI, contribution runtime, and proxy API routes.
- Canonical browser scout runtime path: `web/src/lib/scout-draft.ts` (work polling + draft submission).
- `sdk/python` and `sdk/node`: OpenAI-compatible clients.
- `installers`: platform-specific packaging and onboarding scripts.

## Request Flow
1. Client calls `POST /v1/chat/completions`.
2. Verifier resolves inference mode (`standard` or `speculative`).
3. In speculative mode, verifier enqueues scout work and waits for draft.
4. Scout submits draft to `POST /v1/scout/draft` (PoW-gated).
5. Verifier accepts/rejects draft tokens, then completes generation.
6. Response streams to client via SSE or returns JSON.

## Security Gates
- API key policy is controlled by `SHARD_REQUIRE_API_KEY`.
- `X-Shard-Route: private` always requires API key.
- Scout ingress (`/v1/scout/work`, `/v1/scout/draft`) requires PoW-verified scout identity.
- Signed-envelope routes enforce signature + replay nonce checks.

## Runtime Version Surface
- Root release version source: `VERSION`.
- Daemon exposes version in `/health` (`rust_version`).
- Web and SDK package versions are synchronized from `VERSION` in CI.

## Model Rollout
- Daemon supports canary verifier rollout with traffic splitting and automatic rollback.
- Runtime status is exposed through `/v1/system/model-rollout`.
- Scheduler and chat runtime enforce draft/verifier compatibility pairs before speculative execution.

## Decision Records
- ADR index: `docs/adr/README.md`

## Operational Hardening
- Telemetry WebSocket (`/telemetry/ws`) supports optional token auth via `SHARD_TELEMETRY_WS_TOKEN`.
- Telemetry fan-out is rate-controlled with `SHARD_TELEMETRY_WS_MAX_CONNECTIONS` (default: 64).
- Speculative draft wait defaults are TTFT-oriented (`SHARD_SCOUT_TIMEOUT_MS` defaults to 1500ms and is dynamically bounded by queue/scout health).
- Default CORS policy is local-only unless explicitly overridden via `SHARD_CORS_ORIGINS`.
