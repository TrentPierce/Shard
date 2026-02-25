# TODO

## P0/P1 follow-ups

- [ ] Redact endpoint IP/port details from default daemon logs; keep full addresses behind debug/trace only.
- [ ] Add multi-seed bootstrap policy (`N>=3`) and document operator guidance for resilient decentralized startup.
- [ ] Add stale-bootstrap eviction + peer quality scoring for browser peer persistence.
- [ ] Expand scout ingress protections with per-scout adaptive rate windows and circuit breaking metrics.
- [ ] Add dashboard card for `total tokens generated` wired to authoritative backend counter.
- [ ] Add benchmark automation job that exports TTFT/inter-token/protocol-success/error-distribution artifacts per run.
- [ ] Add CI matrix gate that validates scout reconnect behavior under synthetic bootstrap outages.
