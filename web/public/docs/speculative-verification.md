# Distributed Speculative Verification Loop (Phase 3)

## Control-Plane Messages (Proto)

- `WorkRequest { request_id, prompt_context, min_tokens }`
- `WorkResponse { request_id, peer_id, draft_tokens[], latency_ms }`

## Cooperative Loop (Python Driver)

```text
async cooperative_generate(prompt):
    generated = []
    request_id = new_request_id()

    while not done:
        local_token = local_model.next_token(generated, prompt)
        yield local_token
        generated.append(local_token)

        every 50ms:
            context = last_100_tokens(generated)
            rust.broadcast_work(request_id, context, min_tokens=5)

        draft = rust.try_pop_result(request_id)
        if draft:
            accepted, correction = verify_with_bitnet(generated, draft.draft_tokens)
            yield accepted...
            if correction:
                yield correction
```

## Rust Sidecar Behavior

1. Receives `BroadcastWork` from Python control plane.
2. Publishes `WorkRequest` onto gossipsub topic `shard-work`.
3. Collects `WorkResponse` from peers (`shard-work-result`).
4. Forwards first valid response to Python callback queue.

## Browser Scout Behavior

1. Subscribe to `shard-work`.
2. If local double-dip lock is active, ignore work.
3. Run WebLLM draft generation for 5 tokens.
4. Publish `WorkResponse` on `shard-work-result`.
