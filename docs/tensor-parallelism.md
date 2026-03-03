# Tensor Parallelism Design

## Layer Split Strategy
- Embedding, RMSNorm, and output projection layers are replicated on every verifier.
- Attention heads are sharded across `N` co-verifiers (`heads_per_node = total_heads / N`).
- FFN intermediate channels are sharded across `N` co-verifiers with identical partition boundaries as attention blocks.
- Residual stream shape remains identical on every node to keep merge points deterministic.

## Communication Pattern
- Protocol: `/shard/tensor-parallel/1.0.0`.
- For each transformer block:
  - Leader verifier sends shard-specific layer work to each co-verifier.
  - Co-verifiers return partial logits.
  - Leader performs ring-allreduce equivalent (sum + normalize) over partial logits.
- Timeout handling is per co-verifier request using `co_verifier_timeout_ms`.

## Minimum Viable Configuration
- `degree = 2` nodes.
- Node A: first half of attention heads + FFN channels.
- Node B: second half of attention heads + FFN channels.
- Both nodes keep replicated embedding/output layers.

```yaml
tensor_parallel:
  enabled: false
  degree: 2
  co_verifier_timeout_ms: 500
```

## Why This Beats Single Node Above Throughput Threshold
Let:
- `T_single` = single-node verifier tokens/sec.
- `f_parallel` = fraction of verifier compute that is parallelizable (attention + FFN).
- `N` = tensor parallel degree.

Amdahl estimate:

`speedup = 1 / ((1 - f_parallel) + f_parallel / N)`

With `f_parallel ~= 0.85` and `N=2`:

`speedup ~= 1 / (0.15 + 0.425) = 1.74x`

After network overhead, practical speedup remains >1.3x once request load crosses saturation of a single verifier.

## Failure Handling
- If no `co-verifier` peers are discovered, coordinator falls back to single-node verification.
- If any co-verifier request errors or times out, coordinator aborts parallel merge and falls back to single-node verification for that request.
- Fallback reason is logged and attached to the verification result for observability.
