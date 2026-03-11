## Remote Llama Scout Result — March 11, 2026

This note records the first clean remote browser-scout result on a compatible
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

Three local-only verifier runs:

- `9712 ms`
- `9735 ms`
- `9475 ms`

Verifier trace summary:

- generation time:
  - `8893 ms`
  - `8901 ms`
  - `8627 ms`
- accepted speculative tokens: `0`

### Distributed Results

Three remote browser-scout runs:

- `9469 ms`
- `8883 ms`
- `8512 ms`

Verifier trace summary:

- mailbox hit:
  - `647 ms`
  - `633 ms`
  - `638 ms`
- accepted speculative tokens:
  - `8/8`
  - `8/8`
  - `8/8`
- verify time:
  - `1935 ms`
  - `1985 ms`
  - `2008 ms`
- generation time after accepted draft:
  - `6865 ms`
  - `6251 ms`
  - `5851 ms`

### What This Proves

The remote browser scout path is working correctly on a compatible Llama pair:

- scout attaches successfully
- verifier issues leases
- draft arrives in time
- verifier accepts the full draft window
- the distributed path beat the local-only baseline on all three runs

On this sample, the distributed path improved wall-clock latency by roughly:

- `243 ms`
- `852 ms`
- `963 ms`

Average improvement was about `686 ms` across the three runs.

### Caveats

This is a promising result, but it is not yet the final public benchmark claim.

- The distributed responses were somewhat shorter than baseline:
  - baseline completion tokens: `64`, `64`, `62`
  - distributed completion tokens: `57`, `52`, `49`
- The sample size is still small (`3 vs 3`)
- The setup used a temporary tunnel and a manually prepared remote scout host

### Safe Current Claim

The safe project claim after this run is:

> On a compatible Llama draft/verifier pair, a remote browser scout can
> repeatedly produce accepted speculative tokens and improve wall-clock latency
> on a slower local verifier.

### Recommended Follow-up

- Run a larger repeated set (`10 vs 10`)
- Keep mesh forwarding disabled for the local-vs-local verifier comparison
- Use the same prompt and compare completion-token counts closely
- Only update public benchmark numbers after the larger repeated set confirms
  the same trend
