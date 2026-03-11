# Experimental WAN Llama Scout Test Runbook

Use this runbook when a second machine with WebGPU is available and you want to benchmark the experimental WAN browser-scout path against a local Llama verifier.

This is not the default product path. The normal product flow is local-first browser routing plus desktop verifier escalation.

## Goal

Measure correctness and wall-clock behavior for the verified-compatible pair:

- Browser draft: `meta-llama/Llama-3.2-1B`
- Local verifier: `meta-llama/Llama-3.1-8B`

## Interpretation Before You Start

The experimental WAN path is already known to be correct on this compatible Llama pair.

The latest same-machine live-site benchmark on March 11, 2026 showed:

- baseline: `11295.1 ms` average, `11297 ms` median
- experimental WAN: `12004.4 ms` average, `11888 ms` median
- correctness: `10/10` wait hits and `4/4` accepted tokens on every distributed run

That means the benchmark target has shifted:

- correctness is already proven
- the open question is whether a true no-contention remote scout can beat the verifier-only baseline honestly

## Current Local Verifier Target

- Local daemon URL: `http://127.0.0.1:9191`
- Local GGUF path:
  `E:\lmstudio-models\lmstudio-community\Meta-Llama-3.1-8B-Instruct-GGUF\Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf`
- Temporary tunnel URL:
  `https://<your-tunnel>.trycloudflare.com`

## Before You Start

1. Confirm the local daemon is healthy:

```powershell
curl.exe -s http://127.0.0.1:9191/health
curl.exe -s http://127.0.0.1:9191/v1/system/scout-config
```

2. Confirm the tunnel still reaches the daemon:

```powershell
curl.exe -s https://<your-tunnel>.trycloudflare.com/health
```

3. Reset speculative trace before each measured batch:

```powershell
curl.exe -s -X POST http://127.0.0.1:9191/v1/system/speculative-trace/reset
```

## Scout URL

Open this exact pattern on the scout machine:

`https://shardnetwork.live/benchmark/scout?backend=https://<your-tunnel>.trycloudflare.com`

Wait for:

- `State: ready`
- `Runtime: registered`
- `Capability: webgpu`

Important:

- Do not add `draft_model=qwen` for Llama tests.
- The default benchmark scout page already uses the Llama draft model.
- If the page fails with `Invalid ShaderModule`, switch browsers, enable hardware acceleration, and prefer the discrete GPU.

## Baseline Request

Use a direct local no-scout baseline first.

Request body:

```json
{
  "model": "meta-llama/Llama-3.1-8B",
  "stream": false,
  "max_tokens": 64,
  "messages": [
    {
      "role": "user",
      "content": "Write one short paragraph explaining why peer-to-peer AI networks matter."
    }
  ]
}
```

Send it with:

```powershell
curl.exe -s http://127.0.0.1:9191/v1/chat/completions ^
  -H "Content-Type: application/json" ^
  -H "x-shard-inference-mode: standard" ^
  --data-binary "@tmp_llama_request.json"
```

Record:

- wall-clock latency
- completion tokens
- response quality

## Experimental WAN Request

Once the remote scout page is ready, send the same request with experimental WAN enabled:

```powershell
curl.exe -s http://127.0.0.1:9191/v1/chat/completions ^
  -H "Content-Type: application/json" ^
  -H "x-shard-inference-mode: experimental_wan" ^
  --data-binary "@tmp_llama_request.json"
```

Legacy alias `distributed` is still accepted, but use `experimental_wan` in new runs so the intent is explicit.

## Benchmark Runner

For repeated comparisons, use:

`scripts/dev/run_remote_llama_comparison.ps1`

Recommended controls:

- same prompt
- same seed
- mesh forwarding disabled
- speculative trace reset between batches

## Collect After Every Distributed Batch

```powershell
curl.exe -s http://127.0.0.1:9191/metrics/summary
curl.exe -s http://127.0.0.1:9191/v1/system/speculative-trace
curl.exe -s http://127.0.0.1:9191/health
```

Also collect browser console timing lines from the scout page:

- `[scout-timing]`
- `prefill_ms`
- `decode_ms`
- `submit_ms`
- `reuse`

## What Success Looks Like

The run is only meaningful if all of these happen:

- `active_browser_sessions > 0`
- `active_scouts > 0`
- `draft_capable_scouts > 0`
- speculative trace includes:
  - `lease_issued`
  - `wait_hit_mailbox`
  - `verify_completed`
- metrics show:
  - `speculative_wait_hits_total > 0`
  - `speculative_verify_attempts_total > 0`
  - `speculative_accepted_tokens_total > 0`
- browser logs show timing fields for generation and reuse

## Timing Interpretation

- `prefill_ms` dominates:
  - prompt-state reuse or prompt shortening is still the best browser win
- `decode_ms` dominates:
  - the draft model itself is the limiting factor
- `submit_ms` dominates:
  - transport or backend submission is the problem
- `reuse=exact_prompt_cache` or another reuse mode:
  - the scout is skipping repeated identical browser work successfully

## Common Outcomes

### `dispatch_skipped_zero_timeout`

The daemon decided the scout wait was not profitable. Recheck timeout overrides and low-supply caps.

### `wait_timeout`

The scout got work but did not return the draft in time. Recheck the effective timeout in the speculative trace before changing request size or model settings.

### `verify_zero_accept`

The draft arrived, but the verifier rejected all draft tokens. Treat this as a compatibility issue first, not a networking issue.

### Garbled output with nonzero acceptance

Stop the run and treat it as a model-pair correctness bug. Do not use those numbers for performance claims.

## Current Interpretation Rules

- Fast-node neutrality on Fly is already proven.
- Compatible Llama experimental WAN correctness is already proven.
- Same-machine live-site runs still show the experimental WAN path losing on wall clock.
- Do not update public performance claims unless a repeated remote no-contention run beats the verifier-only baseline with comparable completion length and no quality regression.
