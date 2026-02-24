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
- Key alert triggers:
  - error-rate increase
  - latency regression
  - speculative acceptance collapse
  - PoW failure spikes

## Incident Handling
1. Confirm health and metrics endpoints.
2. Check auth/PoW failure counters.
3. Drain failing nodes from scheduling path.
4. Roll back release if SLO breach persists.
5. Record timeline and corrective actions.

