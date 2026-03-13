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

## Research Workflow Provenance

The v1 agent surface is intentionally opinionated around `research_brief`. It returns an
execution summary, append-only receipts, and a reconstructable provenance graph. Failed
workflow submissions still return the persisted receipt chain and provenance bundle so the
unhappy path remains debuggable. Completed workflow results also include planner notes,
sub-questions, and the source IDs selected for synthesis.

```python
from shard import ShardClient

with ShardClient(base_url="http://localhost:9091") as client:
    task = client.agents.submit(
        question="What should Shard emphasize in its launch narrative?",
        sources=[
            {
                "id": "market-notes",
                "title": "Market notes",
                "content": "Teams care about cross-topology routing clarity and failure visibility.",
            },
            {
                "id": "operator-notes",
                "title": "Operator notes",
                "content": "Contributors value specialist work that improves their own local workflows first.",
            },
        ],
        policy={
            "allowed_supply_tiers": ["personal", "private", "public"],
            "trust_tier": "verified_mesh",
            "capability_tags": ["planning", "summarization", "synthesis"],
            "fallback_order": ["personal", "private", "public"],
            "budget_limit": 1.25,
            "deadline_ms": 45_000,
            "max_public_spend": 0.35,
        },
    )

    execution_id = task.execution.execution_id
    print(task.execution.status)
    print(task.detail)
    print(task.provenance.incomplete)
    print(task.execution.result.sub_questions if task.execution.result else [])

    receipts = client.agents.receipts(execution_id)
    provenance = client.agents.provenance(execution_id)
    capabilities = client.agents.capabilities()

    print(f"receipts={len(receipts)} nodes={len(provenance.nodes)} capabilities={len(capabilities)}")
```

Useful agent methods:

- `client.agents.submit(...)`
- `client.agents.status(execution_id)`
- `client.agents.receipts(execution_id)`
- `client.agents.provenance(execution_id)`
- `client.agents.capabilities()`

If you omit `policy`, the SDK sends the same product defaults used by the Shard web demo:

- `trust_tier = verified_mesh`
- `budget_limit = 1.25`
- `deadline_ms = 45000`
- `capability_tags = ["planning", "summarization", "synthesis"]`
- `fallback_order = ["personal", "private", "public"]`
- `max_public_spend = 0.35`

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
