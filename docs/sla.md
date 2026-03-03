# Shard Service Level Agreement (SLA)

## Latency SLA
- Requests with responses under 500 tokens: `p95 <= 3.0s` and `p99 <= 8.0s`.
- Requests with responses between 500 and 2000 tokens: `p95 <= 8.0s`.
- Measurement window: sampled every 30 seconds from request receipt to final token delivery.
- Source of truth: Prometheus request duration histograms and router telemetry.

## Quality SLA
- Output quality must remain within **2 percentage points** of a 70B baseline on:
  - MMLU (5-shot)
  - HellaSwag
- Evaluation cadence: monthly.
- Sample size: 1,000 production-like requests per month.
- Baseline and benchmark reports are retained for audit and trend analysis.

## Availability SLA
- Monthly uptime objective: **99.5%** per calendar month.
- Planned maintenance exclusion:
  - Maximum 4 hours/month.
  - 24-hour advance notice required.
- Force majeure events are excluded from SLA calculations.
- Availability is calculated using health and successful completion telemetry across the overflow router and Shard verifier path.

## Fallback Policy
- If real-time monitoring detects an SLA latency breach (`p95 > threshold`), the overflow router immediately routes all new requests to the primary backend.
- Cooling period duration: 5 minutes.
- During fallback, clients continue receiving responses without a surfaced error.
- Every fallback event increments `shard_sla_breach_total` and is logged with timestamp, breaker state, and routing reason.

## Measurement and Reporting
- SLA metrics are collected in Prometheus and sampled every 30 seconds.
- Weekly SLA reports are generated from Prometheus query-range data.
- Weekly report includes:
  - Breach counts
  - Weekly p95
  - Bootstrap uptime proxy
  - Acceptance rate
  - Routing split (primary vs Shard)
- Reports are reviewed in operations cadence and attached to incident follow-up when any SLA target is missed.
