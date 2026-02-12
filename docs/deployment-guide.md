# Deployment Guide

## Local Production-Like Launch

1. Start Rust sidecar:
   ```bash
   shard-daemon --control-port 9091 --tcp-port 4001 --bootstrap /ip4/<seed>/tcp/4001
   ```
2. Start Python API:
   ```bash
   cd desktop/python
   SHARD_API_KEYS=prod-key SHARD_RATE_LIMIT_PER_MINUTE=120 python run.py --rust-url http://127.0.0.1:9091
   ```

## Container Deployment

Build:
```bash
docker build -t shard:latest .
```

Run:
```bash
docker run --rm -p 8000:8000 -p 9091:9091 -p 4001:4001 \
  -e SHARD_API_KEYS=prod-key \
  -e SHARD_RATE_LIMIT_PER_MINUTE=120 \
  shard:latest
```

## Monitoring

- API health: `GET /health`
- Network topology: `GET /v1/system/topology`
- Peer list: `GET /v1/system/peers`
- Prometheus-style counters: `GET /metrics`

## Zero-Downtime Update Pattern

- Run at least two API instances behind a load balancer.
- Use rolling replacement (max unavailable = 0).
- Drain traffic from old instances before termination.
