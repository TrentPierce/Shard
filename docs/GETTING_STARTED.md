# Getting Started

This repository contains the full Shard stack (daemon, web client, SDK, benchmarks, integrations, and deployment assets).

## 1. Run the verifier daemon

```bash
docker compose up --build shard-daemon -d
curl http://localhost:9091/health
```

## 2. Run the web Scout UI

```bash
cd web
npm install
npm run dev
```

Open `http://localhost:3000`.

## 3. Run the core validation suite

```bash
pytest tests/test_verification.py --cov --cov-fail-under=95
pytest sdk/python/tests/ --cov=sdk/python/shard --cov-fail-under=90 -q
pytest tests/integration/test_failover.py -v
pytest tests/test_credit_system.py -v
cd desktop/rust && cargo test -p shard-daemon -- --nocapture
```

## 4. Run overflow integration stack

```bash
docker compose -f docker-compose.overflow.yml up -d --build
curl http://localhost:8080/health
```

## Key docs

- `docs/architecture.md`
- `docs/deployment.md`
- `docs/verification-protocol.md`
- `docs/tensor-parallelism.md`
- `docs/enterprise-vpc-deployment.md`
- `docs/sla.md`
