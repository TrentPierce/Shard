## Experimental WAN Llama Results - March 11, 2026

This note records the March 11, 2026 benchmark position for the compatible Llama browser-draft and local verifier pair after the local-first product pivot.

## Summary

Two things are true at the same time:

1. The compatible Llama experimental WAN path is correct and repeatable.
2. It is still not the product fast path, because the latest measured same-machine comparison was slower overall than verifier-only baseline.

## Compatible Pair

- Browser draft: `meta-llama/Llama-3.2-1B`
- Local verifier: `meta-llama/Llama-3.1-8B`
- Local GGUF:
  `E:\lmstudio-models\lmstudio-community\Meta-Llama-3.1-8B-Instruct-GGUF\Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf`

## Result A: Remote No-Contention Compatibility Pass

This was the first clean repeated remote browser-scout result on the compatible Llama pair.

### Setup

- verifier host: local Windows PC
- scout host: remote laptop browser
- scout page: benchmark scout page on `shardnetwork.live`
- tunnel: temporary Cloudflare tunnel to the local verifier
- measurement mode: non-streaming chat completions
- important control: `x-shard-mesh-forward: false`

### Baseline Results

Ten local-only verifier runs:

- average: `10039.765 ms`
- median: `10006.350 ms`
- min: `9787.725 ms`
- max: `10413.472 ms`

### Experimental WAN Results

Ten remote scout runs:

- average: `9890.574 ms`
- median: `9936.093 ms`
- min: `9555.004 ms`
- max: `10217.994 ms`

Verifier trace summary:

- mailbox hit: about `723-939 ms`
- accepted speculative tokens: `8/8` on all `10/10` runs
- verify time: about `2296-2531 ms`

### Interpretation

This pass proved the compatible remote browser scout path works end to end:

- scout attaches successfully
- verifier issues leases
- draft arrives in time
- verifier accepts the full draft window
- no garbled output was observed

This was an important correctness milestone, but it was not strong enough to become the long-term product architecture by itself.

## Result B: Same-Machine Live-Site Validation After Timing Split

This was the more important architecture result, because it used the live scout page with the new timing split and prompt-state reuse instrumentation.

### Setup

- verifier host: local Windows PC
- scout page: `https://shardnetwork.live/benchmark/scout?backend=http://127.0.0.1:9191&draft_model=llama`
- measurement mode: deterministic `10 vs 10`
- seed: `42`
- mesh forwarding: disabled
- browser session state: ready and registered

### Baseline Results

Ten verifier-only runs:

- average: `11295.1 ms`
- median: `11297 ms`
- min: `10495 ms`
- max: `13598 ms`

### Experimental WAN Results

Ten distributed runs:

- average: `12004.4 ms`
- median: `11888 ms`
- min: `11485 ms`
- max: `12978 ms`

Verifier trace summary:

- `10/10` wait hits
- `10/10` verification attempts
- `4/4` accepted draft tokens on every distributed run
- no garbled output

Wall-clock delta versus baseline:

- `+709.3 ms` slower on average
- `+591 ms` slower at the median

### Browser Timing Split

First distributed request:

```text
[scout-timing] ... lease_age_ms=228 generate_ms=377 prefill_ms=258 decode_ms=119 submit_ms=9 total_ms=389 draft_chars=15 success=true reuse=none detail=draft queued
```

Repeated identical prompt requests:

```text
[scout-timing] ... generate_ms=0 prefill_ms=0 decode_ms=0 submit_ms=5-9 total_ms=6-10 ... reuse=exact_prompt_cache
```

### Interpretation

This pass showed four important things:

- the compatible Llama scout path remains correct
- the same-machine experimental WAN path is still slower overall than verifier-only baseline
- browser-side prompt reuse is working
- once the prompt is identical, browser draft generation can collapse close to zero

That timing result supports the architecture pivot: keep WAN scouts experimental and move the shipping product toward local-first browser answers plus desktop heavy inference.

## Safe Current Claim

The safe public claim after the March 11, 2026 benchmark set is:

> On a compatible Llama draft and verifier pair, the experimental WAN scout path is correct and repeatable, but the shipping product path should remain local-first because the latest controlled same-machine run was still slower overall than verifier-only baseline.

## Next Useful Follow-Up

- repeat the remote no-contention benchmark with the new timing split
- compare completion lengths closely between baseline and experimental WAN runs
- benchmark verifier-local speculative mode against `standard`
- keep public performance claims separate for:
  - local browser answers
  - verifier-local speculative execution
  - experimental WAN scouts
