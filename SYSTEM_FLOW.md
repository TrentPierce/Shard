# System Flow Audit (Current State)

## Objective
Document the actual end-to-end flow implemented in this repository today, including gateway, scheduler, scout, shard, verification, networking, and signature-related paths.

## 1. Node Boot and Identity Flow
1. Daemon starts (`desktop/rust/src/main.rs`).
2. Node identity is loaded/created from `identity.json` (`desktop/rust/src/crypto/identity.rs`).
3. Ed25519 key material is used for:
- libp2p peer identity (transport/network identity)
- node "wallet" address derivation (pubkey hex; should be renamed NodeIdentity/PeerKey)
4. HTTP control plane is started with routes such as `/health`, `/topology`, `/peers`, `/v1/chat/completions`, `/broadcast-work`, `/submit-draft`, `/pop-result`, `/metrics/latency-profile`.
5. libp2p swarm starts with gossipsub + request/response protocols.

## 2. Gateway Flow (Current)
Current gateway behavior is effectively in the daemon itself (not a separate service/module):
1. Client request hits `POST /v1/chat/completions`.
2. Daemon validates request shape and enters generation loop.
3. Daemon may enqueue/broadcast work for scouts and process inbound responses.
4. Response is streamed/returned from daemon path.

Note: Many docs still describe a Python gateway API at `/v1/system/*` and `/v1/scout/*`, but this repository currently routes directly through Rust daemon endpoints.

## 3. Scheduler Flow (Current)
A dedicated scheduler module does not exist yet. Scheduling is implicit:
1. Work is published to gossipsub `shard-work`.
2. Responses from `shard-work-result` are consumed and filtered.
3. Race coordination for layer-forward path uses `RaceRouter` (`desktop/rust/src/mesh/race_router.rs`).
4. Scout quality filtering uses penalty/blackhole logic in `ScoutPenaltyBook`.

Current scheduler signals used:
- peer penalty/blackhole status
- race timeout and pool size
- basic per-request timing

Missing for target design:
- weighted multi-factor scoring (load, latency history, reliability, hardware profile, identity reputation)
- deterministic, explicit scheduler interface

## 4. Scout Flow (Current)
In web client:
1. Browser scout polls/fetches work endpoints and/or subscribes via browser libp2p path.
2. Draft text/tokens generated with WebLLM helper.
3. Draft submitted via HTTP API calls.

Issues:
- Scout ID may be random/local-storage generated in web path, not identity-key bound.
- No universal signed result envelope enforced before acceptance.

## 5. Shard/Verification Flow (Current)
1. Daemon receives scout drafts from queue/gossipsub.
2. Drafts are applied in local logic and may trigger credit tx creation in ledger.
3. `verify` request-response protocol exists but currently does not perform full cryptographic verification flow for work results (events logged only).

Net effect:
- Verification exists functionally for some model/result logic.
- Cryptographic verification of every job/result exchange is not consistently enforced.

## 6. Networking Layer Flow
libp2p behaviors in daemon:
- gossipsub topics for work/results/pipeline/ledger/auction
- request/response protocols:
  - handshake
  - verify
  - control_work
  - ledger_sync
- kademlia for provider discovery
- identify/ping/autonat/relay/dcutr behaviors

Connection lifecycle:
1. connect/discover peer
2. send handshake ping
3. mark peer verified after handshake response path
4. maintain peer state and known peer list

## 7. Ledger and Signature-Related Flow
1. Node signs reward/credit transactions via Ed25519 (`LedgerState::sign_reward_tx`).
2. Ledger verifies signature + nonce before accepting tx (`apply_signed_tx`).
3. Ledger state persisted and can sync over request/response protocol.

Important distinction:
- Ledger tx signing is implemented.
- Job dispatch/result messages are not uniformly signed and verified end-to-end yet.

## 8. Observability Flow (Current)
- Telemetry websocket emits periodic snapshot (connected peers, active scouts estimate, derived TFLOPS estimate).
- HTTP latency profile endpoint exposes percentile data from internal histogram.
- No Prometheus-native endpoint currently exported from daemon.

## 9. Failure Behavior (Current)
Implemented:
- peer blackholing based on scout penalty updates
- race timeout and late-result rejection in race router
- reconnect loops and known-peer persistence

Missing for target state:
- deterministic fallback contract with explicit timeout policy and idempotent replay
- signed fallback execution artifacts
- zero request loss guarantees with durable request queue semantics

## 10. Flow Gaps vs Requested End State
- Gateway/scheduler/verification boundaries are not explicit modules.
- End-to-end signed job/result protocol not mandatory today.
- Identity-bound reputation is partial (penalties exist, but not unified score model).
- Metrics required for benchmark/cost reporting are incomplete.
- Deployment topology in docs/scripts assumes components that are absent in current repo.
