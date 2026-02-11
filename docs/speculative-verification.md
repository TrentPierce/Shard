# Distributed Speculative Verification Loop (Phase 4: Fuzzy Judge)

## Control-Plane Messages (Proto)

- `WorkRequest { request_id, prompt_context, min_tokens, sequence_id }`
- `WorkResponse { request_id, peer_id, draft_tokens[], latency_ms, sequence_id }`

## Soft Verification Rule

Instead of exact token matching, accept a scout token if it appears in Oracle top-k (k=3).

## Cooperative Loop (Python Driver)

```text
async cooperative_generate(prompt_tokens):
    sequence_id = 0

    while not eos:
        broadcast_work(context=tokens[-100:], sequence_id=sequence_id, min_draft_len=5)

        try bid in 50ms:
            if bid.sequence_id == sequence_id:
                accepted = verify_draft_batch(top_k=3)
                stream(accepted)
                sequence_id += len(accepted)
                continue
            else:
                discard stale/future bid

        fallback_token = local_generate_one()
        stream(fallback_token)
        sequence_id += 1
```

## Rust Sidecar Behavior

1. Receives `BroadcastWork` from Python control plane.
2. Publishes `WorkRequest` onto gossipsub topic `shard-work`.
3. Collects `WorkResponse` from peers (`shard-work-result`).
4. Forwards responses (with sequence_id) to Python callback queue.

## Browser Scout Behavior

1. Service worker checks local Oracle topology at startup.
2. If local Oracle exists, stop scout inference (double-dip lock).
3. Otherwise, process `shard-work`, generate draft tokens, and return `WorkResponse` with sequence_id.
