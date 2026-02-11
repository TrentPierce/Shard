# Phase 5: Production Hardening

## Hard Shim ABI

The dedicated shard engine shared library exports a fixed C ABI:

- `shard_init(const char*)`
- `shard_free(void*)`
- `shard_eval(void*, const int*, int)`
- `shard_get_logits(void*, float*, int)`
- `shard_rollback(void*, int)`
- `shard_get_vram_usage(void*)`

## Unified Build Chain

Use one entrypoint:

```bash
python scripts/build_release.py
```

Or to skip PyInstaller:

```bash
python scripts/build_release.py --skip-freeze
```

## Release Tests

Run against binaries:

```bash
pytest tests/release_test.py
```

## Deterministic Dependency Strategy (offline/proxy-friendly)

Prepare dependency artifacts once on a machine with external access:

```bash
./scripts/prepare_offline_deps.sh
```

This creates:

- `.offline/python-wheels/` for pip wheelhouse installs.
- `.offline/npm-cache/` for npm offline cache reuse.
- `.offline/cargo-vendor/` for vendored Rust crates.

Suggested install commands in restricted environments:

```bash
# Python
python -m pip install --no-index --find-links .offline/python-wheels -r desktop/python/requirements.txt

# npm
npm ci --cache .offline/npm-cache --prefer-offline

# cargo (with vendor source configured)
cargo build --frozen
```

## CI Gates

The repository CI (`.github/workflows/ci.yml`) now enforces:

1. Python unit/failure-path tests for cooperative inference behavior.
2. Reproducible web installs via `npm ci`.
3. Release-bundle contract checks (`tests/release_test.py`).
