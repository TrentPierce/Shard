# Contributing to Shard

First, thank you for your interest in contributing to Shard! We appreciate your help in building a fast, private, and distributed inference network.

## Contributor License Agreement (CLA)

Before we can merge your pull requests, you must sign the Shard Project Contributor License Agreement (CLA). The CLA is located in the root `CLA.md` file. By submitting a PR, you confirm that your contribution is covered by the CLA.

## Pull Request Process

1. Fork the repository and create your feature branch: `git checkout -b feature/my-new-feature`
2. Commit your changes. Be sure to write clear commit messages.
3. Make sure to run the tests and lints!
   - Rust daemon: `cd desktop/rust && cargo test --all-targets` and `cargo clippy -- -D warnings`
   - Web frontend: `cd web && npm test`
4. Push to the branch: `git push origin feature/my-new-feature`
5. Open a Pull Request on GitHub.

## Adding Features

Please check the existing issues before starting on a new feature. For major changes, we recommend opening an issue first to discuss your proposed approach and ensure it aligns with the project vision.

## Setting Up the Development Environment

See `docs/deployment.md` for information on setting up Shard, including the web dashboard and daemon.
