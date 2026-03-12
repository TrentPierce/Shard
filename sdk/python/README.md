# shardnetwork-client

Python SDK for the Shard distributed inference network.

## Install

```bash
pip install shardnetwork-client
```

## Quickstart

```python
from shard import ShardClient

with ShardClient(base_url="http://localhost:9091") as client:
    print(client.node.status())
    print(client.metrics.summary())
```

## Programmatic Contribution

The SDK can also register a contributor against the daemon's signed control-plane endpoints.

```python
from shard import ShardClient

with ShardClient(base_url="http://localhost:9091") as client:
    contributor = client.contribution.create_session()
    print("public key:", contributor.public_key_hex)

    contributor.set_participation(True)
    contributor.register_node(role="verifier", capacity=1)
    contributor.heartbeat(
        role="verifier",
        queue_depth=0,
        node_latency_ms=24,
        uptime_seconds=12,
        capability_tier="gpu_fast",
        gpu_available=True,
        public_api=True,
    )
    contributor.report_metrics(
        role="verifier",
        queue_depth=1,
        node_latency_ms=32,
        uptime_seconds=30,
        capability_tier="gpu_fast",
        gpu_available=True,
        public_api=True,
    )
    contributor.deregister_node(role="verifier")
```

Persist the generated `seed_hex` if you want to keep the same contributor identity across runs.
