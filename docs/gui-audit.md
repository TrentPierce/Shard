# Shard GUI Audit

Date: 2026-03-04

Scope:
- `desktop/rust/shard-gui`
- daemon startup path used by GUI
- operator prerequisites/docs for contributing a node
- Docker support needed for multi-node performance testing

## Audit Results

1. GUI startup path is valid.
- `shard-gui` starts/stops daemon in-process.
- Model auto-download is wired, with settings fallback for manual model path.
- Telemetry panel pulls health, topology, metrics, ledger, and credits endpoints.

2. Contribution-mode dependency handling is present.
- GUI sets `SHARD_REQUIRE_ENGINE_FOR_CONTRIBUTE=false` when model/library are missing, so node can still run in scout-compatible mode.
- Once model path is available, GUI restarts daemon and contribution telemetry updates.

3. Gap found: model manifest is not production-populated.
- `deploy/models/manifest.json` still contains `REPLACE_WITH_ACTUAL_URL` and placeholder hash/size values.
- Result: first-run auto-download path is not yet turnkey without manually setting model path.

4. Gap found: Docker image healthcheck dependency.
- Compose healthchecks call `wget`, but daemon image previously did not install it.
- Fixed by adding `wget` to `Dockerfile.daemon`.

5. Gap found: runnable local mesh scaling workflow.
- Existing cluster compose file in `deploy/demo` was not turnkey for local scaling.
- Added a dedicated local mesh compose + startup scripts to support 1/2/N node tests quickly.

## Files Added/Updated For Gaps

- `Dockerfile.daemon` (installs `wget` for healthchecks)
- `deploy/demo/docker-compose.mesh.yml` (scalable local mesh)
- `deploy/demo/mesh-up.ps1` (Windows bootstrap + scale helper)
- `deploy/demo/mesh-up.sh` (bash bootstrap + scale helper)
- `docs/mesh-benchmark.md` (operator test workflow)
