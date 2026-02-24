# ADR 0003: Capability-Aware Scheduling and Auditability

- Status: Accepted
- Date: 2026-02-24

## Context
Scheduler decisions were not externally auditable and lacked explicit recorded inputs.

## Decision
Use weighted selection by load, latency, reliability, hardware capability, and identity reputation.
Persist bounded decision logs in-memory and expose read-only audit endpoint:
- `GET /v1/system/scheduler-decisions`

## Consequences
- Enables operational and security audit of routing behavior.
- Adds lightweight state overhead (bounded to 500 entries).

