# Troubleshooting Guide

## API Returns 401
- Confirm `SHARD_API_KEYS` is set correctly.
- Send either `Authorization: Bearer <key>` or `X-API-Key: <key>`.

## API Returns 429
- Request volume exceeded `SHARD_RATE_LIMIT_PER_MINUTE`.
- Increase limit or distribute requests across API instances.

## API Returns 413
- Prompt exceeded `SHARD_MAX_PROMPT_CHARS`.
- Chunk large requests into smaller prompts.

## `rust_sidecar: unreachable`
- Ensure `shard-daemon` is running and reachable at `SHARD_RUST_URL`.
- Verify firewall or container port mappings for control plane (default `9091`).

## No peers discovered
- Pass explicit bootstrap peers with `--bootstrap`.
- Optionally provide `--bootstrap-file` and persistent known peers in data dir.
- Check `/v1/system/topology` and `/v1/system/peers`.

## Streaming fails mid-response
- Check `/metrics` and app logs for `chat_failures_total` spikes.
- Validate `BITNET_LIB` and `BITNET_MODEL` are present and readable.
