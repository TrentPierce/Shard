# Contributing to Shard

First, thank you for your interest in contributing to Shard.

Shard's current product direction is local-first:

- simple prompts should finish in the browser when possible
- harder prompts should escalate cleanly to a desktop verifier
- speculative acceleration should stay inside the verifier boundary by default
- WAN browser scouts should be treated as benchmark-only research, not the default shipping path

Please keep proposals and pull requests aligned with that direction unless the work is explicitly marked experimental.

## Contributor License Agreement (CLA)

Before we can merge your pull requests, you must sign the Shard Project Contributor License Agreement (CLA). The CLA is located in the root `CLA.md` file. By submitting a PR, you confirm that your contribution is covered by the CLA.

## Pull Request Process

1. Fork the repository and create your feature branch: `git checkout -b feature/my-new-feature`
2. Commit your changes. Be sure to write clear commit messages.
3. Make sure to run the tests and lints!
   - Rust daemon: `cd desktop/rust && cargo test --all-targets` and `cargo clippy -- -D warnings`
   - Web frontend: `cd web && npm run lint && npm test -- --passWithNoTests && npm run build`
   - Version parity: `python scripts/verify_versions.py` when release surfaces are touched
4. Push to the branch: `git push origin feature/my-new-feature`
5. Open a Pull Request on GitHub.

## Adding Features

Please check the existing issues and roadmap docs before starting on a new feature. For major changes, open an issue first so the design can be checked against the current local-first architecture and production priorities.

## Setting Up the Development Environment

See `docs/GETTING_STARTED.md` for the quickest local setup, then use `docs/deployment.md` for environment and deployment detail.
