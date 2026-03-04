# Release Candidate Checklist (Public Launch)

This checklist is the go/no-go control document for public release.

## Scope Freeze

- [ ] RC branch created (example: `release/rc1`) and only blocker fixes allowed.
- [ ] Verifier and scout runtime settings frozen for RC window (`deploy/release/rc1.env`).
- [ ] One canonical deployment config is used for both local and EC2 verifiers.

## Environment Parity

- [ ] Both verifier nodes run the same git commit.
- [ ] Both verifier nodes expose healthy `/health` and `/metrics/summary`.
- [ ] Both verifier nodes report the same effective scout runtime config at `/v1/system/scout-config`.
- [ ] Model + engine pair are identical across nodes (or documented if intentionally different).

## Mandatory RC Matrix

Run repeated matrix and auto-generate go/no-go artifacts:

```bash
python benchmarks/distributed/run_release_matrix.py \
  --one-node-pool http://127.0.0.1:9191 \
  --two-node-pool http://127.0.0.1:9191,http://35.175.242.222:9091 \
  --runs-per-scenario 3 \
  --scouts 24 \
  --rate 4 \
  --duration 60 \
  --scout-workers 4
```

Expected outputs in `reports/release-rc/release-rc-<timestamp>/`:

- `go-no-go-summary.json`
- `go-no-go-report.md`
- per-scenario raw run JSON files

## Release Gates (must all pass)

- [ ] `2-node with scouts` p95 latency median `<= 4500ms`.
- [ ] `2-node with scouts` error rate median `<= 3.0%`.
- [ ] `2-node with scouts` throughput median `>= 3.75 TPS`.
- [ ] `2-node with scouts` throughput median `>= 105%` of `2-node no-scouts`.
- [ ] `2-node with scouts` p95 median `<= 110%` of `2-node no-scouts`.

## Reliability and Safety

- [ ] Scout ingress overload behavior verified (`429/503` + `retry_after_ms`).
- [ ] Verifier remains responsive under scout backpressure conditions.
- [ ] No sustained crash/restart loops during soak run.

## Security and Abuse Control

- [ ] PoW challenge/verify flow validated under load.
- [ ] Replay protection active.
- [ ] Scout endpoint rate-limits verified and documented.

## Documentation and Operator Experience

- [ ] README performance snapshot matches latest RC matrix.
- [ ] `docs/release-rc-runbook.md` is updated with rollback conditions and exact commands.
- [ ] Node setup docs validated end-to-end by a clean operator run.

## Final Signoff

- [ ] Go/no-go summary recommendation is `GO`.
- [ ] RC artifact directory is archived and linked in release notes.
- [ ] Rollback owner and on-call owner confirmed for launch window.
