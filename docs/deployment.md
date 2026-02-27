# Deployment

## Required Environment Variables
- `SHARD_BACKEND_URLS` / `NEXT_PUBLIC_SHARD_BACKEND_URLS` (comma/newline/space separated backend list for HA)
- `SHARD_BACKEND_URL` / `NEXT_PUBLIC_SHARD_BACKEND_URL` (single backend fallback)
- `SHARD_FALLBACK_URLS` (optional failover list for chat and API proxy)
- `SHARD_FALLBACK_URL` (single fallback backend)
- `SHARD_CHAT_PRIMARY_TIMEOUT_MS` (chat proxy timeout for primary backends; default `65000`)
- `SHARD_CHAT_FALLBACK_TIMEOUT_MS` (chat proxy timeout for fallback backends; default `90000`)
- `SHARD_REQUIRE_API_KEY` (`true|false`)
- `SHARD_API_KEYS` (comma-separated)
- `SHARD_ADMIN_KEY` (admin API key management endpoint)
- `SHARD_SCOUT_TIMEOUT_MS`
- `SHARD_SCOUT_WORK_QUEUE_MAX` (optional queue cap for scout work fan-out; default `1024`, clamp `64..4096`)
- `SHARD_HEARTBEAT_TIMEOUT_MS`
- `SHARD_ALLOW_PRIVATE_BOOTSTRAP` (`true|false`, default `false`; allows dialing private/loopback bootstrap multiaddrs)
- `SHARD_HARDCODED_BOOTSTRAP_MODE` (`fallback|always|disabled`, default `fallback`; `fallback` only uses built-in bootstrap when no user/bootstrap-url/persisted peers exist)
- `SHARD_BOOTSTRAP_REGISTRY_TTL_MS` (optional TTL for persisted bootstrap registry entries; default `86400000`)
- `SHARD_BOOTSTRAP_REGISTRY_MIN_SCORE` (optional minimum stability score `0..100` for registry-seeded bootstrap addrs; default `30`)
- `SHARD_CANARY_ENABLED`
- `SHARD_CANARY_MODEL_ID`
- `SHARD_CANARY_TRAFFIC_PERCENT`
- `SHARD_CANARY_MAX_AVG_LATENCY_MS`
- `SHARD_CANARY_MIN_ACCEPTANCE_RATE`
- `SHARD_CANARY_MAX_REJECT_RATE`
- `SHARD_CANARY_MIN_SAMPLES`
- `SHARD_PROXY_SLI_HISTORY_MS` (optional retention window for proxy chat SLI samples; default `900000`)
- `SHARD_PROXY_SLI_WINDOW_MS` (optional active SLI window for proxy chat error-rate calculations; default `300000`)
- `SHARD_PROXY_SLI_MIN_WINDOW_REQUESTS` (minimum 5m requests before proxy 5xx SLI breach can trigger; default `20`)
- `SHARD_PROXY_SLI_MAX_5XX_RATE` (proxy 5xx SLI threshold; default `0.05`)

## Core Services
- Daemon: `desktop/rust/daemon`
- Web app: `web`
- Monitoring: `deploy/monitoring/prometheus`, `deploy/monitoring/grafana`

## HA Baseline
- Run at least two publicly reachable shard daemon backends.
- Configure both in `SHARD_BACKEND_URLS` so the web API proxy can fail over automatically.
- Do not rely on a single bootstrap or a single public telemetry endpoint.

## Health Checks
- Daemon health: `GET /health`
- Metrics summary: `GET /metrics/summary`
- Prometheus metrics: `GET /metrics`

## Release Rule
- Release version comes only from root `VERSION`.
- CI blocks merges on version mismatch (`python scripts/verify_versions.py`).

## Windows Signing and Trust State
- Windows releases support two trust states:
  - `signed` (preferred): Sign installers with `installers/windows/sign-installer.ps1` using `signtool.exe` and a trusted Authenticode certificate.
  - `unsigned-supported` preview: Build and publish unsigned artifacts when no trusted cert is available; release output must include explicit unsigned preview disclosure.
- Verification for signed artifacts is performed after signing (`signtool verify /pa`).
- Release workflow signs Windows artifacts when `WIN_CODESIGN_CERT_BASE64` and `WIN_CODESIGN_CERT_PASSWORD` are present; otherwise it emits an unsigned preview marker file in release assets.

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
