# Versioning Policy

## Single Source of Truth
- Root `VERSION` defines the release version.

## Propagation
- `scripts/sync_versions.py` writes version to Rust/web/SDK/installers.
- `scripts/verify_versions.py` fails if any required surface is out of sync.

## Runtime Consistency
- `/health` exposes daemon runtime version.
- Web package and SDK package versions must match root `VERSION` for release builds.

## Release Manifest Standard
- Generate manifest with:
  - `python scripts/generate_release_manifest.py --artifact <path> ...`
- Verify manifest with:
  - `python scripts/verify_release_manifest.py --manifest dist/release-manifest.json --require-signatures`
- Sign artifacts with:
  - `python scripts/sign_release.py <artifact>`
- Manifest ties each artifact checksum to `VERSION`, git tag, and commit.

## Breaking Change Policy
- Backward-compatible changes: patch/minor increments.
- Breaking API or protocol changes: major increment and migration notes in release docs.
