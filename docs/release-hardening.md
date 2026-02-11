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
