# Remote Llama Scout Test Runbook

Use this runbook when a second machine with WebGPU is available and we want to test a real browser scout against the local Llama 8B verifier without same-machine GPU contention.

## Goal

Prove or disprove end-to-end speculative value for the verified-compatible pair:

- Browser draft: `meta-llama/Llama-3.2-1B`
- Local verifier: `meta-llama/Llama-3.1-8B`

## Current local verifier target

- Local daemon URL: `http://127.0.0.1:9191`
- Temporary tunnel URL: `https://specified-sec-cms-often.trycloudflare.com`
- Local GGUF path:
  `E:\lmstudio-models\lmstudio-community\Meta-Llama-3.1-8B-Instruct-GGUF\Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf`

## Before you start

1. Confirm the local daemon is healthy:

```powershell
curl.exe -s http://127.0.0.1:9191/health
curl.exe -s http://127.0.0.1:9191/v1/system/scout-config
```

2. Confirm the tunnel still reaches the daemon:

```powershell
curl.exe -s https://specified-sec-cms-often.trycloudflare.com/health
```

3. Reset speculative trace before each measured run:

```powershell
curl.exe -s -X POST http://127.0.0.1:9191/v1/system/speculative-trace/reset
```

## Laptop scout URL

Open this exact URL on the scout machine:

`https://shardnetwork.live/benchmark/scout?backend=https://specified-sec-cms-often.trycloudflare.com&draft_model=llama`

Wait for:

- `State: ready`
- `Runtime: registered`
- `Capability: webgpu`

If the page fails with `Invalid ShaderModule`, switch browsers, enable hardware acceleration, and prefer the discrete GPU.

## Baseline request

Use a direct local no-scout baseline first.

Create the request body:

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
  --data-binary "@tmp_llama_request.json"
```

Record:

- wall-clock latency
- response quality

## Distributed request

Once the remote scout page is ready, send the same request with distributed inference enabled:

```powershell
curl.exe -s http://127.0.0.1:9191/v1/chat/completions ^
  -H "Content-Type: application/json" ^
  -H "x-shard-inference-mode: distributed" ^
  --data-binary "@tmp_llama_request.json"
```

## Collect after every distributed run

```powershell
curl.exe -s http://127.0.0.1:9191/metrics/summary
curl.exe -s http://127.0.0.1:9191/v1/system/speculative-trace
curl.exe -s http://127.0.0.1:9191/health
```

## What success looks like

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

## Common outcomes

### `dispatch_skipped_zero_timeout`

The daemon decided the scout wait was not profitable. Recheck the local timeout override and the low-supply cap.

### `wait_timeout`

The scout got work but did not return the draft in time. Recheck the effective timeout in the speculative trace before changing request size or model settings.

### `verify_zero_accept`

The draft arrived, but the verifier rejected all draft tokens. Treat this as a compatibility issue first, not a networking issue.

### Garbled output with nonzero acceptance

Stop the run and treat it as a model-pair correctness bug. Do not use those numbers for performance claims.

## Current interpretation rules

- Fast-node neutrality is already proven on Fly.
- Same-machine Llama correctness is already proven locally.
- The missing proof is a remote no-contention Llama run that shows accepted drafts without corrupted output.

Do not update public benchmark claims until that remote Llama run succeeds.
