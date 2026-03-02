# Versioning

Shard uses a single, unified `VERSION` file at the root of the repository to ensure all components of the network (daemon, web app, python SDK, installers, etc.) stay exactly in sync.

When a new version is released:

1. Update the `VERSION` file.
2. Run the sync script: `make version-sync` (or `python scripts/sync_versions.py`)
3. The script automatically updates all package manifests and configurations:
   - `desktop/rust/*/Cargo.toml`
   - `web/package.json` and `web/src/lib/version.ts`
   - `sdk/python/pyproject.toml`
   - Installers and README badges

We enforce matching versions strictly in our CI pipeline via `scripts/verify_versions.py`. This ensures no mismatched client/server versions are published.
