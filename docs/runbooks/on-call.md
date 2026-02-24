# On-Call Runbook

## Rotation Expectations
- Primary responder acknowledges pages within 5 minutes.
- Secondary responder joins within 10 minutes for `SEV-1` and `SEV-2`.

## Pager Decision Tree
1. Confirm alert validity in Grafana + Prometheus.
2. Classify severity and impact scope.
3. If `SEV-1`, escalate immediately to security + platform leads.
4. Execute incident-response runbook steps.

## Standard Commands
- Version check: `python scripts/verify_versions.py`
- Contract checks: `pytest tests/contract_protocol_test.py -q`
- Daemon health: `curl http://127.0.0.1:9091/health`
- Scheduler audit: `curl http://127.0.0.1:9091/v1/system/scheduler-decisions`

## Handover Requirements
- Current incident status and timeline.
- Active mitigations and rollback state.
- Pending risks and next check-in time.

