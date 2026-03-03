# shard-client

Python SDK for the Shard distributed inference network.

## Quickstart

```python
import shard

with shard.Client("http://localhost:9091") as client:
    print(client.node.status())
    print(client.metrics.summary())
```
