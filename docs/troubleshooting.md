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

## `bitnet_loaded: false` on `/health`
- Ensure `SHARD_TESTING=0` in runtime environment.
- Ensure both `BITNET_LIB` and `BITNET_MODEL` are set and point to readable files.
- Ensure `LD_LIBRARY_PATH` includes the directory containing `libllama.so`/`libggml*.so` when using `libshard_engine.so`.
- Restart API service after env changes:
  - `sudo systemctl restart shard-api.service`
- Validate:
  - `curl http://127.0.0.1:8000/health`

## Browser app gets 401 in production
- API key auth is enabled when `SHARD_REQUIRE_API_KEY=true`.
- Add `NEXT_PUBLIC_SHARD_API_KEY` to Vercel environment variables.
- If using Vercel rewrites, confirm `NEXT_PUBLIC_API_URL=/api`.

## Browser scouts cannot connect over websocket
- Use a TLS endpoint (`wss://...`) for browser-facing transport.
- Confirm DNS resolves to the host and TLS cert is valid.
- Verify reverse proxy forwards websocket upgrade headers.
- Confirm daemon advertises a public DNS host (`--public-host`) instead of private IP only.
