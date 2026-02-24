# Deployment

## Required Environment Variables
- `SHARD_BACKEND_URL` / `NEXT_PUBLIC_SHARD_BACKEND_URL`
- `SHARD_REQUIRE_API_KEY` (`true|false`)
- `SHARD_API_KEYS` (comma-separated)
- `SHARD_ADMIN_KEY` (admin API key management endpoint)
- `SHARD_SCOUT_TIMEOUT_MS`
- `SHARD_HEARTBEAT_TIMEOUT_MS`
- `SHARD_CANARY_ENABLED`
- `SHARD_CANARY_MODEL_ID`
- `SHARD_CANARY_TRAFFIC_PERCENT`
- `SHARD_CANARY_MAX_AVG_LATENCY_MS`
- `SHARD_CANARY_MIN_ACCEPTANCE_RATE`
- `SHARD_CANARY_MAX_REJECT_RATE`
- `SHARD_CANARY_MIN_SAMPLES`

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

## Windows Signing
- Sign Windows installers with `installers/windows/sign-installer.ps1`.
- Requires `signtool.exe` and an installed signing certificate.
- Verification is performed after signing (`signtool verify /pa`).
- Release workflow enforces signing secrets for Windows `release` events and verifies Authenticode signatures on produced `.exe` artifacts.

## Windows Auto-Update Channels
- Updater script: `installers/windows/update.ps1`
- Channel config: `installers/windows/update-channels.json`
- Supported channels:
  - `stable`
  - `canary`
- Installer creates scheduled task `ShardAutoUpdate` by default.
- Rollback: updater snapshots current install under `%ProgramData%\Shard\rollback` and restores on failed update.

## Windows First-Run Onboarding
- Script: `installers/windows/first-run.ps1`
- Installer behavior:
  - interactive install runs GUI onboarding wizard
  - silent install runs non-interactive onboarding (`/S /NOGUI`)
- Manual flags:
  - `install.bat /DOWNLOADMODEL` to force model acquisition
  - `install.bat /NOGUI` to force console onboarding
  - `install.bat /NOONBOARD` to skip onboarding
