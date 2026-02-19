# Shard Architecture Audit (2026-02-19)

## Scope
This audit covers the repository as it exists today, with emphasis on runtime code (`desktop/rust`, `web`), deployment files (`docker-compose.yml`, `deploy/*`), and CI (`.github/workflows/*`).

## High-Level Reality Check
The live runtime is currently centered on a single Rust daemon binary (`desktop/rust/src/main.rs`) plus a web UI/scout client (`web/*`).

Several docs and deployment files still describe a Python API gateway in `desktop/python`, but that directory is absent in this repo.

## Current Runtime Components
- `desktop/rust/src/main.rs`: monolithic control-plane daemon.
  - HTTP API, libp2p swarm, gossipsub, request/response protocols, identity loading, ledger crediting, scout penalty tracking, browser-layer routing, and some chat path logic are all in one binary.
- `desktop/rust/src/crypto/*`: Ed25519 node identity + encrypted backup tooling.
- `desktop/rust/src/ledger/*`: signed credit transaction model and persistence.
- `desktop/rust/src/network/*`: layer registry, tensor wire, obfuscation helpers.
- `desktop/rust/src/mesh/race_router.rs`: first-writer race arbitration.
- `desktop/rust/src/telemetry_ws.rs`: websocket telemetry snapshots (limited metric depth).
- `web/src/*`: client UI, browser scout worker loops, p2p client, network visualizers.

## Architecture Weaknesses
1. Core control-plane is monolithic and tightly coupled.
- `desktop/rust/src/main.rs` contains most subsystems and route wiring in one file.
- Hard to reason about failure domains, deterministic behavior, and test isolation.

2. Boundary mismatch between docs/deploy and code.
- Repo references `desktop/python` in `Dockerfile`, `README.md`, many docs, and EC2 scripts, but folder is missing.
- `Dockerfile` references `scripts/docker-entrypoint.sh`, but actual file is `deploy/docker-entrypoint.sh`.

3. Incomplete trust enforcement for job execution path.
- Ed25519 is used for node identity and ledger transactions, but work dispatch/result acceptance in gossipsub/request-response is not consistently signed/verified before acceptance.
- `verify` protocol is declared but currently only logs events.

4. Metrics stack is only partially wired.
- Compose runs Prometheus/Grafana optionally, but Prometheus targets `shard-api:8000`, which is not defined in current compose services.
- Rust exports limited custom JSON metrics endpoints, not Prometheus-native instrumentation.

5. Identity terminology is semantically misleading.
- `wallet` naming is used widely for what is effectively node key identity.
- This blurs distributed infra identity vs token/economic semantics.

6. Test gaps in integration and failure behavior.
- Rust unit tests exist and pass for several utility modules.
- Web tests currently fail in Node due Tauri runtime assumptions (`window.__TAURI_INTERNALS__`).
- No end-to-end tests covering gateway->scheduler->node->verification with signed messages.

## Experimental / Dead / At-Risk Areas
- `desktop/control_plane/shard_control.proto`: comprehensive control-plane spec appears not wired into runtime.
- `Dockerfile`: presently broken against current repository layout (missing python path + wrong entrypoint path), likely stale.
- `web/src/lib/discovery.ts`: depends on external community/IPFS paths and `'/v1/system/topology'` assumptions not aligned with current daemon routes.
- `web/src/components/NetworkVisualizer.tsx`: fetches absolute `"/v1/system/peers"` and `"/health"` (bypassing `apiUrl`), which is deployment fragile.
- `web/src/hooks/useSwarmTelemetry.ts`: contains TODO placeholders for token tracking and synthetic TFLOPS estimates.

## Current Component Mapping vs Target Boundaries
Current:
- Mixed in `main.rs`: gateway, scheduler-like behavior, execution node logic, verification hooks, networking, metrics, identity, ledger.

Target (requested):
- `/gateway`
- `/scheduler`
- `/execution_nodes`
- `/verification`
- `/network`
- `/metrics`
- `/identity`
- `/common`

Status:
- `identity`, parts of `network`, and parts of `execution_nodes` exist but not cleanly separated.
- `gateway`, `scheduler`, and `verification` are currently implicit behaviors in `main.rs`, not first-class modules.

## Test/CI Snapshot
- Rust CI checks run formatting, clippy, and tests (`.github/workflows/ci.yml`).
- No coverage gate, no integration test matrix, no failure/chaos suite.
- Web CI build/test exists, but local web test run currently fails without extra Tauri mocking.

## Primary Refactor Risk
The largest risk is modifying behavior while untangling `main.rs`. Refactoring must be staged behind explicit interfaces and parity tests to avoid regressions in transport behavior and identity handling.

## Audit Completion Gate
This audit is complete for Phase 1.1 and supports moving into incremental boundary refactors defined in `REFACTOR_PLAN.md`.
