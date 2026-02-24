# Incident Response Runbook

## Severity Levels
- `SEV-1`: customer-impacting outage or security compromise.
- `SEV-2`: major degradation with workaround.
- `SEV-3`: localized or low-blast-radius issue.

## Immediate Actions
1. Declare incident in on-call channel and assign incident commander.
2. Capture start time, symptoms, and active alerts.
3. Stabilize first: traffic shed, rollback, or isolate failing nodes.
4. Preserve evidence: logs, metrics snapshots, config versions.
5. Communicate status every 15 minutes until mitigated.

## Technical Triage Checklist
1. Check `/health`, `/metrics/summary`, and `/metrics`.
2. Validate auth/PoW/signature failure counters.
3. Inspect scheduler decision logs (`/v1/system/scheduler-decisions`).
4. Confirm bootstrap registry/mesh connectivity (`/v1/system/bootstrap`).
5. Decide rollback vs forward fix.

## Containment Patterns
- High latency: reduce speculative traffic, increase fallback budget.
- PoW failure spike: increase challenge difficulty and rate-limit ingress.
- Mesh instability: remove unhealthy bootstrap peers and rebalance routes.

## Recovery and Exit
1. Verify SLO signals are back in bounds for 30 minutes.
2. Announce incident resolution and monitor for recurrence.
3. Start post-incident review within 24 hours.

