# Model Upgrade and Rollback Strategy

## Rollout Stages
1. Staging validation with signed artifacts.
2. Canary rollout on low traffic percentage.
3. Promotion to stable after SLO pass window.

## Guardrails
- Monitor latency, reject rate, and fallback rate.
- Abort promotion if SLO breaches are sustained.

## Rollback
- Keep last known-good model manifest.
- Trigger rollback automatically on alert thresholds.
- Verify post-rollback health and speculative metrics before closing incident.

