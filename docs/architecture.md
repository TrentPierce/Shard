# Shard Architecture

## Product Direction

Shard is now organized around a local-first execution rule:

1. Answer easy prompts in the browser when possible.
2. Escalate harder prompts to a desktop verifier daemon.
3. Keep speculative decoding inside the verifier boundary by default.
4. Treat WAN browser scouts as an explicit experimental path, not the default product loop.

## Components

- `desktop/rust/daemon`: heavy inference worker, scheduler, mesh networking, and ingress APIs.
- `web`: local-first chat app, browser runtime, conversation state, prompt compaction, and benchmark scout UI.
- `web/src/lib/browser-router.ts`: heuristic browser router that returns `local_answer`, `network_route`, or `network_route_with_compaction`.
- `web/src/lib/conversation-state.ts`: browser-owned chat history and assistant token assembly.
- `web/src/lib/prompt-compaction.ts`: browser-side summary and trimming for long conversations.
- `web/src/lib/webllm.ts`: browser local-answer runtime plus experimental scout runtime.
- `web/src/lib/scout-capability.ts`: browser accelerator detection for current WebGPU runtime support plus the low-power ONNX/WebNN worker lane.
- `web/src/lib/webnn-embeddings.ts` and `web/src/lib/webnn-embeddings-worker.ts`: semantic ranking and compaction helpers that prefer ONNX/WebNN, then fall back to ONNX/WASM or hashed embeddings.
- `web/src/lib/swarm.ts` and `web/src/lib/scout-engine.ts`: experimental scout lifecycle and timing instrumentation.
- `desktop/rust/daemon/src/scheduler.rs`: inference-mode resolution, verifier scheduling, and speculative routing rules.

## Default Request Flow

1. A user submits a prompt in the browser chat UI.
2. The browser router scores the prompt using prompt size, conversation size, and complexity heuristics.
3. The router chooses one of three outcomes:
   - `local_answer`
   - `network_route`
   - `network_route_with_compaction`
4. If the request stays local, WebLLM generates the answer entirely in-browser.
5. If the request escalates, the browser sends raw or compacted messages to `POST /v1/chat/completions`.
6. The verifier daemon resolves one of three internal inference modes:
   - `Standard`
   - `LocalSpeculative`
   - `ExperimentalWanSpeculative`
7. The daemon returns the final response through the same OpenAI-compatible API.

## Inference Modes

### `standard`

- Verifier-only generation.
- This is the clean baseline path.
- Mesh forwarding can be considered here when the request is eligible.

### `local_speculative`

- Explicit opt-in network acceleration path.
- Keeps speculative work inside the verifier boundary instead of waiting on a remote browser draft.
- This is where future desktop-local draft-plus-target acceleration should continue to evolve.
- It remains opt-in until it beats `standard` on the target hardware class.

### `experimental_wan`

- Explicit benchmark-only mode for browser-scout experiments.
- Enabled by `X-Shard-Inference-Mode: experimental_wan`.
- Legacy headers `distributed` and `speculative` are still accepted as aliases for backward compatibility.
- This mode is not used by normal product chat unless the user explicitly selects `Experimental WAN`.

## Browser-Owned Conversation State

The browser is the source of truth for session context.

- Raw chat turns live in browser memory.
- Older turns can be compacted into a summary plus a recent-turn window before network escalation.
- The daemon receives a request-sized prompt payload, not long-lived user session ownership.
- Model weights stay hot in the verifier, but request-specific KV and scratch state are not treated as durable session state.

This is intentionally different from trying to serialize browser KV cache into a different verifier runtime. Shard keeps the browser-side state model-agnostic.

## Browser Accelerator Classification and Semantic Worker Lane

Shard now distinguishes between two browser capability classes:

- interactive browser runtime: currently WebGPU/WebLLM only
- low-power background worker lane: ONNX/WebNN when available, with ONNX/WASM or hashed fallback

This matters because the current browser generation path is still GPU-backed, while browser-side semantic ranking and compaction can now run in a lower-risk worker lane without claiming full browser generation on WebNN.

Today:

- WebGPU is the only shipped browser inference runtime.
- ONNX/WebNN is the shipped semantic worker path for embeddings-style ranking and compaction tasks.
- The worker prefers `webnn`, falls back to `wasm`, and then to deterministic hashed embeddings if runtime initialization fails.
- The browser still performs capability probing and caches warm-state so repeated sessions do not re-pay avoidable probe cost.

## Experimental WAN Scout Path

The browser-scout path still exists, but it is separated from the default chat architecture.

1. A benchmark scout page boots the browser runtime explicitly, usually under `/benchmark/scout`.
2. The scout polls `GET /v1/scout/work`.
3. The scout generates a short draft and submits it to `POST /v1/scout/draft`.
4. The verifier accepts or rejects the draft and completes generation.

This path is currently for:

- compatibility testing
- timing instrumentation
- benchmark research

It is not the default fast path for end users.

## Mesh and Routing

- Verifier nodes still participate in a libp2p mesh for bootstrap, health sharing, and request forwarding.
- Mesh forwarding is a verifier-side optimization for non-speculative routes.
- Short and latency-sensitive work should prefer the healthiest fast verifier tier.
- The scorer now keeps recent endpoint history, including actual forward latency, probe freshness, and cooldown state, so slow-but-alive peers are penalized instead of being retried blindly.
- Speculative modes stay local to the selected verifier because token-level WAN coordination has a poor latency ceiling.

## Security Gates

- API key policy is controlled by `SHARD_REQUIRE_API_KEY`.
- `X-Shard-Route: private` always requires an API key.
- Experimental scout ingress (`/v1/scout/work`, `/v1/scout/draft`) requires PoW-verified scout identity.
- Signed-envelope routes enforce signature and replay-nonce checks.

## Runtime Version Surface

- Root release version source: `VERSION`.
- Daemon exposes version in `/health` as `rust_version`.
- Web and SDK package versions are synchronized from `VERSION` in CI.

## Operational Notes

- `NEXT_PUBLIC_ENABLE_EXPERIMENTAL_WAN_SCOUT` is disabled by default for product sessions.
- `/benchmark/scout` remains the explicit entry point for WAN scout tests.
- Scout timing now logs `prefill_ms`, `decode_ms`, `submit_ms`, and `reuse` to separate browser generation cost from transport overhead.
- Browser capability surfaces can now report `backgroundAcceleration`, `lowPowerEligible`, `webnnProbeMs`, and `webnnWarmState` without changing the default WebGPU runtime path.

## Decision Records

- ADR index: `docs/adr/README.md`
