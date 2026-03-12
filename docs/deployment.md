# Deployment

## Product Defaults

Normal deployments should use the local-first product path:

- keep `NEXT_PUBLIC_ENABLE_EXPERIMENTAL_WAN_SCOUT` unset or `false`
- use `/chat` for product traffic
- use the verifier daemon as the heavy-work backend
- use `/benchmark/scout` only for explicit experimental WAN tests

Experimental scout tuning should not be part of the default production startup path.

## Required Environment Variables

- `SHARD_BACKEND_URLS` / `NEXT_PUBLIC_SHARD_BACKEND_URLS` for HA backend lists
- `SHARD_BACKEND_URL` / `NEXT_PUBLIC_SHARD_BACKEND_URL` for a single backend fallback
- `SHARD_FALLBACK_URLS` for optional failover backends
- `SHARD_FALLBACK_URL` for a single fallback backend
- `SHARD_CHAT_PRIMARY_TIMEOUT_MS` for primary chat proxy timeout, default `65000`
- `SHARD_CHAT_FALLBACK_TIMEOUT_MS` for fallback chat proxy timeout, default `90000`
- `SHARD_REQUIRE_API_KEY` as `true|false`
- `SHARD_API_KEYS` as a comma-separated key list
- `SHARD_ADMIN_KEY` for admin API key management
- `SHARD_SCOUT_TIMEOUT_MS`
- `SHARD_SCOUT_WORK_QUEUE_MAX` optional queue cap for scout work fan-out, default `1024`, clamp `64..4096`
- `SHARD_SCOUT_BACKPRESSURE_START_QUEUE_DEPTH` default `256`
- `SHARD_SCOUT_BACKPRESSURE_LATENCY_WARN_MS` default `3000`
- `SHARD_SCOUT_BACKPRESSURE_LATENCY_SEVERE_MS` default `6000`
- `SHARD_SCOUT_ADMISSION_QUEUE_DEPTH` default `5`
- `SHARD_SCOUT_ADMISSION_QUEUE_HARD_DEPTH` default `10`
- `SHARD_SCOUT_ADMISSION_LATENCY_SOFT_MS` default `4500`
- `SHARD_SCOUT_ADMISSION_LATENCY_HARD_MS` default `6000`
- `SHARD_SCOUT_POLL_MIN_INTERVAL_MS` default `75`
- `SHARD_SCOUT_DRAFT_MIN_INTERVAL_MS` default `50`
- `SHARD_SCOUT_ACTIVE_CAP` default `8`
- `SHARD_SCOUT_ACTIVE_CAP_SOFT` default `4`
- `SHARD_SCOUT_ACTIVE_CAP_HARD` default `2`
- `SHARD_MESH_FORWARD_ENABLED` as `true|false`, default `true`
- `SHARD_MESH_FORWARD_MAX_HOPS` default `1`, clamp `0..4`
- `SHARD_MESH_FORWARD_PROBE_LIMIT` default `4`
- `SHARD_MESH_FORWARD_PROBE_TIMEOUT_MS` default `600`
- `SHARD_MESH_FORWARD_TIMEOUT_MS` default `20000`
- `SHARD_MESH_FORWARD_QUEUE_WEIGHT_MS` default `120`
- `SHARD_MESH_FORWARD_MIN_IMPROVEMENT_MS` default `120`
- `SHARD_MESH_FORWARD_LOCAL_QUEUE_TRIGGER` default `2`
- `SHARD_RELEASE_PROFILE` optional label shown by `/v1/system/scout-config`
- `SHARD_HEARTBEAT_TIMEOUT_MS`
- `SHARD_ALLOW_PRIVATE_BOOTSTRAP` as `true|false`, default `false`
- `SHARD_HARDCODED_BOOTSTRAP_MODE` as `fallback|always|disabled`, default `fallback`
- `SHARD_BOOTSTRAP_REGISTRY_TTL_MS` optional TTL for persisted bootstrap registry entries, default `86400000`
- `SHARD_BOOTSTRAP_REGISTRY_MIN_SCORE` optional minimum stability score `0..100`, default `30`
- `SHARD_CANARY_ENABLED`
- `SHARD_CANARY_MODEL_ID`
- `SHARD_CANARY_TRAFFIC_PERCENT`
- `SHARD_CANARY_MAX_AVG_LATENCY_MS`
- `SHARD_CANARY_MIN_ACCEPTANCE_RATE`
- `SHARD_CANARY_MAX_REJECT_RATE`
- `SHARD_CANARY_MIN_SAMPLES`
- `SHARD_PROXY_SLI_HISTORY_MS` optional retention window for proxy chat SLI samples, default `900000`
- `SHARD_PROXY_SLI_WINDOW_MS` optional active SLI window for proxy chat error-rate calculations, default `300000`
- `SHARD_PROXY_SLI_MIN_WINDOW_REQUESTS` minimum request count before proxy 5xx SLI breach can trigger, default `20`
- `SHARD_PROXY_SLI_MAX_5XX_RATE` proxy 5xx SLI threshold, default `0.05`

## Web Product Flags

- `NEXT_PUBLIC_ENABLE_EXPERIMENTAL_WAN_SCOUT`
  - default `false`
  - keeps background WAN browser scouts off for normal product sessions
- `NEXT_PUBLIC_PREFER_LOCAL_SHARD`
  - when `true`, the browser prefers a localhost verifier if one is detected
- `NEXT_PUBLIC_ENABLE_BROWSER_LAYER_HOST`
  - experimental browser layer-host path
- `NEXT_PUBLIC_ENABLE_BROWSER_P2P`
  - experimental browser libp2p path

## Experimental WAN Controls

These settings matter only when benchmarking the experimental WAN scout path:

- `SHARD_SCOUT_TIMEOUT_MS`
- `SHARD_SCOUT_WORK_QUEUE_MAX`
- `SHARD_SCOUT_BACKPRESSURE_*`
- `SHARD_SCOUT_ADMISSION_*`
- `SHARD_SCOUT_POLL_MIN_INTERVAL_MS`
- `SHARD_SCOUT_DRAFT_MIN_INTERVAL_MS`
- `SHARD_SCOUT_ACTIVE_CAP*`

They should not be treated as the primary production optimization surface for the local-first product loop.

## Core Services

- Daemon: `desktop/rust/daemon`
- Web app: `web`
- Monitoring: `deploy/monitoring/prometheus`, `deploy/monitoring/grafana`

## Cloudflare Pages

- Production web project name: `shard`
- Production deploys are expected to come from the `main` branch via Cloudflare Pages Git integration
- Verify that Pages has picked up the latest `main` commit with:

```bash
npm run cf:pages:verify --prefix web
```

- List recent Pages deployments with:

```bash
npm run cf:pages:list --prefix web
```

## HA Baseline

- Run at least two publicly reachable shard daemon backends
- Configure both in `SHARD_BACKEND_URLS` so the web API proxy can fail over automatically
- Do not rely on a single bootstrap or public telemetry endpoint
- Keep mesh forwarding enabled on verifier nodes so standard requests can spill into healthier peers
- Keep model weights hot on verifier nodes; browser sessions should own chat history and compaction before escalation

## Health Checks

- Daemon health: `GET /health`
- Metrics summary: `GET /metrics/summary`
- Prometheus metrics: `GET /metrics`
- Scout runtime config and admission state: `GET /v1/system/scout-config`

## Release Candidate Config Freeze

- Canonical RC profile: `deploy/release/rc1.env`
- Keep local and EC2 verifier nodes on the same commit and same RC env file while running the RC matrix

### Apply Frozen Config in Local Docker Mesh

The mesh compose file is the canonical local verifier path and already loads `deploy/release/rc1.env` plus `deploy/release/benchmark.env` via `env_file`.

```bash
powershell -ExecutionPolicy Bypass -File deploy/demo/mesh-up.ps1 -Nodes 2
curl http://localhost:19091/v1/system/scout-config
```

### Apply Frozen Config on EC2 systemd Daemon

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

- Release version comes only from root `VERSION`
- CI blocks merges on version mismatch with `python scripts/verify_versions.py`

## Windows Signing and Trust State

- Windows releases support two trust states:
  - `signed`: sign installers with `installers/windows/sign-installer.ps1` using `signtool.exe` and a trusted Authenticode certificate
  - `unsigned-supported` preview: publish unsigned artifacts when no trusted certificate is available
- Verification for signed artifacts is performed after signing with `signtool verify /pa`
- Release workflow signs Windows artifacts when `WIN_CODESIGN_CERT_BASE64` and `WIN_CODESIGN_CERT_PASSWORD` are present; otherwise it emits an unsigned preview marker file

## Windows Auto-Update Channels

- Updater script: `installers/windows/update.ps1`
- Channel config: `installers/windows/update-channels.json`
- Supported channels:
  - `stable`
  - `canary`
- Installer creates scheduled task `ShardAutoUpdate` by default
- Rollback: updater snapshots current install under `%ProgramData%\Shard\rollback` and restores on failed update

## Windows First-Run Onboarding

- Script: `installers/windows/first-run.ps1`
- Installer behavior:
  - interactive install runs GUI onboarding wizard
  - silent install runs non-interactive onboarding with `/S /NOGUI`
- Manual flags:
  - `install.bat /DOWNLOADMODEL` to force model acquisition
  - `install.bat /NOGUI` to force console onboarding
  - `install.bat /NOONBOARD` to skip onboarding
