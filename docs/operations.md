# Operations Runbook

## SLIs and SLOs
- Availability SLI: `% successful /v1/chat/completions` responses.
  - SLO: `99.9%` monthly.
- Latency SLI: `p95` end-to-end latency.
  - SLO: `p95 < 2500ms`.
- Speculative quality SLI: draft acceptance and reject rate.
  - SLO: acceptance `>= 0.60`, reject `<= 0.40`.
- Throughput SLI: tokens/sec and queue depth.
  - SLO: queue depth under steady-state threshold.

## Dashboards and Alerts
- Grafana dashboard: `deploy/monitoring/grafana/dashboards/shard-operations.json`
- Prometheus config: `deploy/monitoring/prometheus/prometheus.yml`
- Prometheus alert rules: `deploy/monitoring/prometheus/alerts.yml`
- Key alert triggers:
  - error-rate increase
  - latency regression
  - speculative acceptance collapse
  - PoW failure spikes
  - zero-token drift despite successful requests
  - scout submit success-rate degradation

### Paging Thresholds
- `ShardHighTaskFailureRate`: page if task failures > 20/5m for 5m.
- `ShardHighP95Latency`: page if p95 latency > 2500ms for 10m.
- `ShardSpeculativeAcceptanceDrop`: warn if acceptance rate < 0.60 for 10m.
- `ShardPowFailureSpike`: warn if PoW failures > 30/10m.
- `ShardZeroTokenDrift`: page if successful chat completions occur but total token counters remain zero for 5m.
- `ShardScoutSubmitDegraded`: warn if scout submit success rate drops below 20% over 10m with at least 20 submit attempts.

### Alert-to-Runbook Mapping
- Ops alerts -> `docs/operations.md`
- Security alerts -> `docs/security.md`
- Model rollout controls -> `GET /v1/system/model-rollout`, `POST /v1/system/model-rollout/reset-rollback`

## Incident Handling
1. Confirm health and metrics endpoints.
2. Check auth/PoW failure counters.
3. Check browser contribution telemetry event stream (`shard:contribution-status`) and session state for non-contributing reason codes.
4. Validate token drift alert inputs: `shard_chat_completion_success_total` and `shard_tokens_processed_total + shard_tokens_offloaded_to_scouts_total`.
5. Validate scout ingress health: submit attempt/success/failure counters and active draft-capable scout counts.
6. Drain failing nodes from scheduling path.
7. Roll back release if SLO breach persists.
8. Record timeline and corrective actions.

## Runbooks
- Incident response: `docs/runbooks/incident-response.md`
- On-call: `docs/runbooks/on-call.md`
- Latest tabletop outcome: `docs/runbooks/tabletop-2026-02-24.md`
- Benchmark proof protocol: `benchmarks/README.md`
