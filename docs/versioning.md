# Versioning Policy

## Single Source of Truth
- Root `VERSION` defines the release version.

## Propagation
- `scripts/sync_versions.py` writes version to Rust/web/SDK/installers.
- `scripts/verify_versions.py` fails if any required surface is out of sync.

## Runtime Consistency
- `/health` exposes daemon runtime version.
- Web package and SDK package versions must match root `VERSION` for release builds.

## Breaking Change Policy
- Backward-compatible changes: patch/minor increments.
- Breaking API or protocol changes: major increment and migration notes in release docs.

