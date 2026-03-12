# Release Candidate Checklist (Public Launch)

This checklist is the go/no-go control document for public release.

## Scope Freeze

- [ ] RC branch created (example: `release/rc1`) and only blocker fixes allowed.
- [ ] Product runtime settings frozen for RC window (`deploy/release/rc1.env`).
- [ ] One canonical deployment config is used for both local and EC2 verifiers.

## Environment Parity

- [ ] Both verifier nodes run the same git commit.
- [ ] Both verifier nodes expose healthy `/health` and `/metrics/summary`.
- [ ] Both verifier nodes report the same effective routing- and mesh-sensitive runtime config.
- [ ] Model + engine pair are identical across nodes (or documented if intentionally different).

## Mandatory Product Validation

Run the normal shipping path first:

```bash
curl http://127.0.0.1:9091/health
curl http://127.0.0.1:9091/metrics/summary
```

Minimum required product checks:

- [ ] Browser `Auto` mode answers simple prompts locally.
- [ ] Browser `Auto` mode escalates clearly complex prompts to the verifier path.
- [ ] `Network Only` succeeds against the verifier path.
- [ ] `standard` remains healthy as the default verifier-routed path.
- [ ] `local_speculative` remains available as an explicit opt-in path.
- [ ] Mixed browser-local and verifier-routed requests complete without correctness regressions.
- [ ] Multi-backend failover remains healthy under one-backend outage or cooldown.

## Stability Gates (must all pass)

- [ ] Browser-local p95 and verifier-routed p95 remain within the RC budget for the scenario set under test.
- [ ] Error rate stays within release budget for both browser-local and network-routed requests.
- [ ] No sustained crash or restart loops occur during the soak window.
- [ ] Backend failover and mesh forwarding remain responsive when one verifier is degraded.
- [ ] Health, metrics, and version surfaces remain truthful throughout the run.
- [ ] Signed contributor-control endpoints remain healthy for SDK-based node contribution.

## Desktop-Local Speculative Gate

Validate `local_speculative` against `standard` as a separate product-performance gate:

- [ ] `local_speculative` beats or matches `standard` at p50 and p95 on the target hardware class.
- [ ] No garbled-output or correctness regressions appear when speculative mode is enabled.
- [ ] Any uplift claim is backed by repeated controlled runs, not one warm-cache sample.

## Experimental WAN Appendix

Experimental WAN scouts are not a ship/no-ship requirement.

- [ ] If the experimental WAN path is benchmarked, results are documented separately.
- [ ] Public performance claims do not use experimental WAN numbers unless the path beats the simpler product baseline in repeated runs.

## Security and Abuse Control

- [ ] PoW challenge/verify flow validated under load.
- [ ] Replay protection active.
- [ ] Experimental scout endpoint rate-limits are verified and documented if those endpoints are exposed at all in the RC.

## Documentation and Operator Experience

- [ ] README and architecture docs match the current local-first product path.
- [ ] `docs/release-rc-runbook.md` is updated with rollback conditions and exact commands.
- [ ] Node setup docs validated end-to-end by a clean operator run.

## Final Signoff

- [ ] Go/no-go recommendation is `GO` for the product path.
- [ ] Benchmark artifacts are archived and linked in release notes when used for decision support.
- [ ] Rollback owner and on-call owner confirmed for launch window.
