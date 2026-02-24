# Deployment

## Required Environment Variables
- `SHARD_BACKEND_URL` / `NEXT_PUBLIC_SHARD_BACKEND_URL`
- `SHARD_REQUIRE_API_KEY` (`true|false`)
- `SHARD_API_KEYS` (comma-separated)
- `SHARD_ADMIN_KEY` (admin API key management endpoint)
- `SHARD_SCOUT_TIMEOUT_MS`
- `SHARD_HEARTBEAT_TIMEOUT_MS`

## Core Services
- Daemon: `desktop/rust/daemon`
- Web app: `web`
- Monitoring: `deploy/monitoring/prometheus`, `deploy/monitoring/grafana`

## Health Checks
- Daemon health: `GET /health`
- Metrics summary: `GET /metrics/summary`
- Prometheus metrics: `GET /metrics/prometheus`

## Release Rule
- Release version comes only from root `VERSION`.
- CI blocks merges on version mismatch (`python scripts/verify_versions.py`).

