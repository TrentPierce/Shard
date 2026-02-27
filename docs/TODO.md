# TODO

## P0/P1 follow-ups

- [ ] Redact endpoint IP/port details from default daemon logs; keep full addresses behind debug/trace only.
- [ ] Add multi-seed bootstrap policy (`N>=3`) and document operator guidance for resilient decentralized startup.
- [ ] Add stale-bootstrap eviction + peer quality scoring for browser peer persistence.
- [ ] Expand scout ingress protections with per-scout adaptive rate windows and circuit breaking metrics.
- [x] Add dashboard card for `total tokens generated` wired to authoritative backend counter.
- [x] Add scout proxy backend cooldown/load-shedding so `/api/v1/scout/work` and `/api/v1/scout/draft` fail open (200 + transient) instead of hammering backend during outage windows.
- [x] Align default scout/verifier model identity to `meta-llama/Llama-3.2-1B` with backward-compatible alias normalization for legacy `shard-hybrid` / `default-model` requests.
- [ ] Add benchmark automation job that exports TTFT/inter-token/protocol-success/error-distribution artifacts per run.
- [ ] Add CI matrix gate that validates scout reconnect behavior under synthetic bootstrap outages.
- [x] Persist scout participation without manual refresh: if a scout tab stays open, it auto-reconnects across shard daemon restarts/deploys and resumes contribution state/work loop automatically.
- [ ] Reduce EC2 distributed p95 latency and TTFT (target <=5s p95) by eliminating repeated TCP reconnect failures and validating relay/NAT traversal path quality under live WAN load.
