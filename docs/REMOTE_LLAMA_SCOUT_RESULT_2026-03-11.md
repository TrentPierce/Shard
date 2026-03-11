## Remote Llama Scout Result - March 11, 2026

This note records the first clean repeated remote browser-scout result on a compatible
Llama draft/verifier pair.

### Setup

- Verifier host: local Windows PC
- Verifier model: `meta-llama/Llama-3.1-8B`
- Verifier GGUF:
  `E:\lmstudio-models\lmstudio-community\Meta-Llama-3.1-8B-Instruct-GGUF\Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf`
- Scout host: remote laptop browser
- Scout page:
  `https://shardnetwork.live/benchmark/scout?backend=https://remind-complete-jul-remainder.trycloudflare.com`
- Draft model: browser-default Llama WebLLM draft model
- Tunnel: temporary Cloudflare tunnel to the local verifier
- Measurement mode: non-streaming chat completions
- Important control: `x-shard-mesh-forward: false`
  was used for both baseline and distributed requests so the comparison stayed
  on the same local verifier instead of accidentally forwarding into the mesh.

### Prompt

`Write one short paragraph explaining why peer-to-peer AI networks matter.`

### Baseline Results

Ten local-only verifier runs:

- average: `10039.765 ms`
- median: `10006.350 ms`
- min: `9787.725 ms`
- max: `10413.472 ms`

Verifier trace summary:

- completion tokens:
  - mostly `59-61`
- generation time:
  - about `8627-8901 ms`
- accepted speculative tokens: `0`

### Distributed Results

Ten remote browser-scout runs:

- average: `9890.574 ms`
- median: `9936.093 ms`
- min: `9555.004 ms`
- max: `10217.994 ms`

Verifier trace summary:

- mailbox hit:
  - about `723-939 ms`
- accepted speculative tokens:
  - `8/8` on all `10/10` runs
- verify time:
  - about `2296-2531 ms`
- generation time after accepted draft:
  - about `6452-6806 ms`

### What This Proves

The remote browser scout path is working correctly on a compatible Llama pair:

- scout attaches successfully
- verifier issues leases
- draft arrives in time
- verifier accepts the full draft window
- the distributed path produced accepted speculative tokens on all ten runs
- the distributed path beat the local-only median on the repeated set

On this repeated `10 vs 10` sample, the distributed path improved wall-clock
latency by about:

- `149.191 ms` on average
- `70.257 ms` at the median

### Caveats

This is a promising result, but it is not yet the final public benchmark claim.

- The distributed responses were still somewhat shorter than baseline:
  - baseline completion tokens: mostly `59-61`
  - distributed completion tokens: mostly `49`
- The setup used a temporary tunnel and a manually prepared remote scout host

### Safe Current Claim

The safe project claim after this run is:

> On a compatible Llama draft/verifier pair, a remote browser scout can
> repeatedly produce accepted speculative tokens and deliver a small but
> measurable wall-clock latency improvement on a slower local verifier.

### Recommended Follow-up

- Run a larger repeated set than `10 vs 10`
- Keep mesh forwarding disabled for the local-vs-local verifier comparison
- Use the same prompt and compare completion-token counts closely
- Only update public benchmark numbers after the larger repeated set confirms
  the same trend
