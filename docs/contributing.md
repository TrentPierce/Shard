# Contributing

## Development Gates
- Web: lint, build, tests.
- Rust: fmt, clippy, tests.
- Python: `pytest tests/`.
- Version governance: `python scripts/verify_versions.py`.

## Contract Changes
- Update schema files under `docs/schemas/`.
- Update `docs/api.md`.
- Add or adjust contract tests in `tests/contract_protocol_test.py`.

## Security-Sensitive Changes
- Do not bypass PoW, auth, or replay guards.
- Add negative tests for new ingress or trust paths.

## Commit Conventions for Readiness Backlog
- Use atomic commits mapped to ticket IDs, e.g. `P0-T10: enforce PoW on scout ingress`.

