# Shard Architecture

## Components
- `desktop/rust/daemon`: control plane, verifier runtime, mesh networking, and ingress APIs.
- `web`: browser scout UI, contribution runtime, and proxy API routes.
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

## Decision Records
- ADR index: `docs/adr/README.md`
