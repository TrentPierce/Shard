# Model Upgrade and Rollback Strategy

## Rollout Stages
1. Staging validation with signed artifacts.
2. Canary rollout on low traffic percentage.
3. Promotion to stable after SLO pass window.

## Canary Runtime Controls
- `SHARD_CANARY_ENABLED` (`true|false`)
- `SHARD_CANARY_MODEL_ID` (target verifier model id, default `verifier-v2`)
- `SHARD_CANARY_TRAFFIC_PERCENT` (0-100, default `10`)
- `SHARD_CANARY_MAX_AVG_LATENCY_MS` (default `2500`)
- `SHARD_CANARY_MIN_ACCEPTANCE_RATE` (default `0.60`)
- `SHARD_CANARY_MAX_REJECT_RATE` (default `0.40`)
- `SHARD_CANARY_MIN_SAMPLES` (default `20`)

## Rollout Endpoints
- `GET /v1/system/model-rollout`
  - Returns stable model, canary config, status, and live canary stats.
- `POST /v1/system/model-rollout/reset-rollback`
  - Clears automatic rollback status and resets canary counters.
  - Requires `X-Shard-Admin` when `SHARD_ADMIN_KEY` is configured.

## Guardrails
- Monitor latency, reject rate, and fallback rate.
- Abort promotion if SLO breaches are sustained.

## Interface Compatibility
- Verifier runtime now exposes a model abstraction (`VerifierModel`) and compatibility probe (`check_model_compatibility`).
- Health responses surface protocol compatibility (`model_protocol_version`, `model_supports_speculative`) to prevent mixed-node incompatibility during upgrades.
- Draft/verifier compatibility pairs are defined in `docs/model-compatibility-matrix.md` and enforced at runtime by scheduler compatibility checks.

## Rollback
- Keep last known-good model manifest.
- Trigger rollback automatically on alert thresholds.
- Verify post-rollback health and speculative metrics before closing incident.
