# Getting Started

This repository contains the full Shard stack: verifier daemon, local-first web client, benchmark tools, SDKs, and deployment assets.

## 1. Run the verifier daemon

```bash
docker compose up --build shard-daemon -d
curl http://localhost:9091/health
```

## 2. Run the web app

```bash
cd web
npm install
npm run dev
```

Open `http://localhost:3000/chat`.

Use the chat mode selector as follows:

- `Auto`: normal product path
- `Browser Only`: force a local browser answer
- `Network Only`: force the verifier path
- `Experimental WAN`: use only when an explicit benchmark scout is prepared

## 3. Optional: run the experimental scout benchmark

Open `http://localhost:3000/benchmark/scout`.

For the real benchmark procedure, use:

- `docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md`

## 4. Run the core validation suite

```bash
pytest tests/test_verification.py --cov --cov-fail-under=95
pytest sdk/python/tests/ --cov=sdk/python/shard --cov-fail-under=90 -q
pytest tests/integration/test_failover.py -v
pytest tests/test_credit_system.py -v
cd desktop/rust && cargo test -p shard-daemon -- --nocapture
```

## 5. Run overflow integration stack

```bash
docker compose -f docker-compose.overflow.yml up -d --build
curl http://localhost:8080/health
```

## Key Docs

- `docs/architecture.md`
- `docs/deployment.md`
- `docs/api.md`
- `docs/verification-protocol.md`
- `docs/NETWORK_PERFORMANCE_ROADMAP.md`
- `docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md`
