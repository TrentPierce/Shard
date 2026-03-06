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
- `SHARD_SCOUT_BACKPRESSURE_START_QUEUE_DEPTH` (default `256`)
- `SHARD_SCOUT_BACKPRESSURE_LATENCY_WARN_MS` (default `3000`)
- `SHARD_SCOUT_BACKPRESSURE_LATENCY_SEVERE_MS` (default `6000`)
- `SHARD_SCOUT_ADMISSION_QUEUE_DEPTH` (default `5`)
- `SHARD_SCOUT_ADMISSION_QUEUE_HARD_DEPTH` (default `10`)
- `SHARD_SCOUT_ADMISSION_LATENCY_SOFT_MS` (default `4500`)
- `SHARD_SCOUT_ADMISSION_LATENCY_HARD_MS` (default `6000`)
- `SHARD_SCOUT_POLL_MIN_INTERVAL_MS` (default `75`)
- `SHARD_SCOUT_DRAFT_MIN_INTERVAL_MS` (default `50`)
- `SHARD_SCOUT_ACTIVE_CAP` (default `8`)
- `SHARD_SCOUT_ACTIVE_CAP_SOFT` (default `4`)
- `SHARD_SCOUT_ACTIVE_CAP_HARD` (default `2`)
- `SHARD_MESH_FORWARD_ENABLED` (`true|false`, default `true`)
- `SHARD_MESH_FORWARD_MAX_HOPS` (default `1`, clamp `0..4`)
- `SHARD_MESH_FORWARD_PROBE_LIMIT` (default `4`)
- `SHARD_MESH_FORWARD_PROBE_TIMEOUT_MS` (default `600`)
- `SHARD_MESH_FORWARD_TIMEOUT_MS` (default `20000`)
- `SHARD_MESH_FORWARD_QUEUE_WEIGHT_MS` (default `120`)
- `SHARD_MESH_FORWARD_MIN_IMPROVEMENT_MS` (default `120`)
- `SHARD_MESH_FORWARD_LOCAL_QUEUE_TRIGGER` (default `2`)
- `SHARD_RELEASE_PROFILE` (optional label shown by `/v1/system/scout-config`)
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

## Cloudflare Pages
- Production web project name: `shard`
- Production deploys are expected to come from the `main` branch via Cloudflare Pages Git integration.
- Verify that Pages has picked up the latest `main` commit with:

```bash
npm run cf:pages:verify --prefix web
```

- List recent Pages deployments with:

```bash
npm run cf:pages:list --prefix web
```

## HA Baseline
- Run at least two publicly reachable shard daemon backends.
- Configure both in `SHARD_BACKEND_URLS` so the web API proxy can fail over automatically.
- Do not rely on a single bootstrap or a single public telemetry endpoint.
- Keep mesh-forwarding enabled on verifier nodes so queue pressure can spill into healthier peers.

## Health Checks
- Daemon health: `GET /health`
- Metrics summary: `GET /metrics/summary`
- Prometheus metrics: `GET /metrics`
- Scout runtime config and admission state: `GET /v1/system/scout-config`

## Release Candidate Config Freeze
- Canonical RC profile: `deploy/release/rc1.env`
- Keep local and EC2 verifier nodes on the same commit and same RC env file while running the RC matrix.

### Apply frozen config in local Docker mesh
The mesh compose file is the canonical local verifier path and already loads
`deploy/release/rc1.env` plus `deploy/release/benchmark.env` via `env_file`.

```bash
powershell -ExecutionPolicy Bypass -File deploy/demo/mesh-up.ps1 -Nodes 2
curl http://localhost:19091/v1/system/scout-config
```

### Apply frozen config on EC2 systemd daemon
```bash
sudo mkdir -p /etc/shard
sudo cp deploy/release/rc1.env /etc/shard/rc1.env
sudo chmod 0644 /etc/shard/rc1.env
sudo systemctl edit shard-daemon
```

Use these overrides:

```ini
[Service]
EnvironmentFile=/etc/shard/rc1.env
```

```ini
[Service]
EnvironmentFile=/etc/shard/benchmark.env
```

Then reload and verify:

```bash
sudo systemctl daemon-reload
sudo systemctl restart shard-daemon
curl http://127.0.0.1:9091/v1/system/scout-config
curl http://127.0.0.1:9091/health
```

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
