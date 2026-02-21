# Project Audit Report (2026-02-11)

## Overall Rating

**6.5 / 10**

This repository has a coherent architecture and good component boundaries (Python API, Rust sidecar, web UI), but it is currently not release-ready due to missing reproducibility, scaffolded inference paths, and environment-sensitive build/test behavior.

## What Works Well

1. **Clear high-level architecture and docs**
   - The split between Shard API (Python), control plane (Rust), and scout/client surface (web) is clearly documented and implemented.
2. **Reasonable API surface**
   - Health, topology, peers, and OpenAI-style chat endpoints are present.
3. **Release smoke tests exist**
   - `tests/release_test.py` provides a foundation for release bundle validation.

## Blocking Issues

1. **Inference path is still scaffolded**
   - `desktop/python/shard_api.py` still emits hardcoded scaffold tokens and a permissive verifier (`_stub_local_generate`, `_stub_verify`) rather than a real generation/verification flow.
2. **Frontend dependency reproducibility issue**
   - `npm ci` fails because `package.json` and `package-lock.json` are out of sync (`@types/react` mismatch).
3. **Network/proxy fragility for setup**
   - Rust and Python dependency installs fail behind restrictive proxy settings, and there is no offline/locked vendor strategy.

## High-Impact Improvements to Reach 10/10

1. **Replace all scaffolding with production inference + verification**
   - Wire `BitNetRuntime` generation path end-to-end and make speculative verification deterministic/tested.
2. **Fix JS lockfile integrity and enforce CI checks**
   - Ensure `npm ci` works from clean checkout and gate merges with CI.
3. **Add deterministic, offline-friendly builds**
   - For Rust: consider vendored dependencies (`cargo vendor`) for constrained environments.
   - For Python: add a lock file and optional wheelhouse/cached install strategy.
4. **Strengthen API robustness/security**
   - Replace blanket `except Exception` swallowing with structured logging.
   - Tighten CORS and make it environment-configurable.
5. **Expand automated tests beyond release smoke checks**
   - Add unit tests for SSE parsing, cooperative generation edge cases, and control-plane failure handling.

## Validation Run Notes

- `pytest -q` executed, but all release tests were skipped due to missing release bundle artifacts.
- Rust tests could not run due to crates.io proxy restriction (403).
- Python dependency installation could not complete due to pip proxy restriction (403).
- Frontend build could not run because `npm ci` currently fails with lock mismatch.
