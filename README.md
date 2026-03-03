<div align="center">
  <img src="docs/assets/logo.png" alt="Shard" width="180" />
  <h1>Shard Network</h1>
  <p><strong>Distributed speculative decoding with browser scouts and verifier daemons.</strong></p>
</div>

[![CI/CD](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.6.2-blue.svg)](https://github.com/TrentPierce/Shard/releases/tag/v0.6.2)
[![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)

## Overview

Shard is a full-stack distributed inference project:

- `desktop/rust/`: verifier daemon (libp2p mesh, axum APIs, bootstrap ring, policy controls)
- `web/`: Next.js Scout UI and telemetry integration (including WebGPU capability probe)
- `sdk/python/`: typed Python SDK client/resources
- `benchmarks/`: benchmark harness + distributed orchestrator
- `integrations/`: overflow router with circuit breaker and SLA enforcer
- `deploy/`: Docker, Terraform, Kubernetes, and monitoring assets

## Current Status (v0.6.2)

Implemented:

- Phase 1 core deliverables: benchmark harness, WebGPU telemetry path, verification protocol + tests
- Phase 2 hardening: bootstrap ring config/health tooling, credit-system and failover tests
- Phase 3 foundations: distributed orchestrator and tensor-parallel + network-policy modules/tests
- Phase 4 integration: overflow router (`/v1/chat/completions`, `/health`, `/metrics`) with circuit breaker and SLA cooldown enforcement

Most core tests are green locally/CI; scale-gate tuning for 1000-scout drills is being actively optimized.

## Quickstart

Run daemon with Docker:

```bash
docker compose up --build shard-daemon -d
curl http://localhost:9091/health
```

Run web app:

```bash
cd web
npm install
npm run dev
```

Run Python tests:

```bash
pytest tests/test_verification.py --cov --cov-fail-under=95
pytest sdk/python/tests/ --cov=sdk/python/shard --cov-fail-under=90 -q
pytest tests/integration/test_failover.py -v
pytest tests/test_credit_system.py -v
pytest integrations/tests/test_sla.py -v
```

Run Rust tests:

```bash
cd desktop/rust
cargo test -p shard-daemon -- --nocapture
cargo test -p shard-daemon tensor_parallel -- --nocapture
cargo test -p shard-daemon network_policy -- --nocapture
```

Run overflow stack drill:

```bash
docker compose -f docker-compose.overflow.yml up -d --build
curl http://localhost:8080/health
curl http://localhost:8080/metrics
```

## Documentation

- Architecture: `docs/architecture.md`
- Deployment: `docs/deployment.md`
- Verification protocol: `docs/verification-protocol.md`
- Tensor parallelism: `docs/tensor-parallelism.md`
- Enterprise VPC mode: `docs/enterprise-vpc-deployment.md`
- SLA definition: `docs/sla.md`
- Contributing: `docs/contributing.md`

## License

Business Source License 1.1 (BSL 1.1). See `LICENSE`.
