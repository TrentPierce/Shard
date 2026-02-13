# Network Topology and Scaling Characteristics

## Topology Model

- **Shard nodes** run Rust daemon + Python API.
- **Scout nodes** provide draft tokens via gossipsub and req/resp protocols.
- **Consumer clients** connect to Python API (OpenAI-compatible endpoint).

## Bootstrap and Recovery

- Bootstrap sources are merged from:
  - `--bootstrap` CLI flags
  - `--bootstrap-file` entries
  - persisted known peers (`known_peers.json` in local shard data dir)
- Daemon performs periodic redial attempts (`--reconnect-seconds`) to known peers.

## Peer Verification

- On connect, daemon sends `PING` over handshake protocol.
- Peer becomes `verified` on `PONG` or valid inbound heartbeat exchange.
- `/peers` returns verification status and handshake metadata.

## Scaling Characteristics

- Work fan-out uses gossipsub topics `shard-work` and `shard-work-result`.
- Result queue is bounded to control memory pressure.
- For horizontal scale, shard API instances should be stateless and share bootstrap seed pools.

## Recommended Production Baseline

- 3+ bootstrap seeds across failure domains.
- 2+ API replicas behind load balancer.
- Rate limits and API-key auth enabled.
- Continuous `/metrics` scraping with alerting on failure-rate spikes.
