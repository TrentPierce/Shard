# Join The Shard Network

This guide explains what each role does, how to join, and why distributed mode is different from a single EC2 node.

## Roles

### Scout (browser contributor)
- Runs a lightweight draft model in the browser.
- Generates draft token candidates for active requests.
- Fastest way to contribute: open the app with WebGPU enabled.

### Shard (verifier node)
- Runs a full local model and verifies draft tokens.
- Produces final output tokens and enforces correctness.
- Adds real network capacity, reliability, and fault tolerance.

### API Consumer
- Uses the OpenAI-compatible endpoints.
- Benefits from the network when Scouts and Shards are active.

## Single Node vs Distributed

### Single-node mode
- One Shard API + one model (for example on EC2).
- Works reliably as a normal inference server.
- Limited by one machine for throughput and availability.

### Distributed mode
- Many Scouts provide speculative drafts.
- Many Shards verify in parallel and absorb load spikes.
- Better tail latency and resilience when participation is high.

## How To Contribute

1. Open the web app to contribute as a Scout.
2. Run a Shard node using `docs/deployment-guide.md`.
3. Monitor participation and output quality on `/network`.

## Practical Notes

- If no Scouts are connected, the API falls back to local-only generation.
- Distributed gains appear when both Scout participation and Shard capacity are present.
- Model quality on Shard nodes still matters; weak models reduce final answer quality even with many Scouts.
