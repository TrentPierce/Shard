# Enterprise VPC Deployment

## AWS VPC Prerequisites
- Use a dedicated VPC CIDR (example: `10.20.0.0/16`).
- Subnet layout:
  - `3` private subnets across `3` AZs (one bootstrap node per AZ minimum).
  - optional private app subnets for verifier/gateway pools.
- Security group rules (minimum):
  - TCP `4001` from VPC CIDR (libp2p TCP)
  - TCP `9090` from VPC CIDR
  - UDP `9090` from VPC CIDR (WebRTC)
  - UDP `9092` from VPC CIDR (QUIC)
  - TCP `9091` from approved internal CIDRs (control API)
  - outbound all to VPC CIDR (or stricter internal-only policy)

## Private Bootstrap Ring Setup
1. Provision at least 3 bootstrap nodes in different AZs.
2. Start shard-daemon on each node with stable peer identities.
3. Record each node multiaddr and health endpoint.
4. Populate `deploy/config/bootstrap-ring.yaml` with internal IP-based addresses.
5. Set `min_connected_bootstrap` to at least `2`.
6. Validate with `scripts/check-bootstrap-health.sh` from within the VPC.

## Sample network_policy.yaml
```yaml
mode: private
allowed_peer_cidrs:
  - 10.20.0.0/16
  - 172.16.0.0/12
blocked_peer_cidrs: []
allowed_bootstrap_addrs:
  - /ip4/10.20.1.10/tcp/4001/p2p/12D3KooW...
  - /ip4/10.20.2.10/tcp/4001/p2p/12D3KooW...
  - /ip4/10.20.3.10/tcp/4001/p2p/12D3KooW...
reject_public_ips: true
audit_log_blocked_connections: true
```

## Verify No Egress Leaves VPC
1. Enable VPC Flow Logs on subnet and ENI level.
2. Filter for shard instances and inspect destination addresses.
3. Confirm all accepted traffic destinations are private CIDRs.
4. Alert on any `ACCEPT` flow to public internet CIDRs.

## Docker Compose Configuration
```yaml
services:
  shard-daemon:
    image: ghcr.io/trentpierce/shard-daemon:latest
    command: ["--private-mode"]
    environment:
      - SHARD_NETWORK_POLICY_PATH=/config/network_policy.yaml
    volumes:
      - ./deploy/config:/config
```

## Kubernetes Configuration
```yaml
containers:
  - name: shard-daemon
    image: ghcr.io/trentpierce/shard-daemon:latest
    args: ["--private-mode"]
    env:
      - name: SHARD_NETWORK_POLICY_PATH
        value: /config/network_policy.yaml
    volumeMounts:
      - name: shard-config
        mountPath: /config
```

In private mode, the daemon loads `network_policy.yaml`, disables default public bootstrap seeds, and exposes `"private_mode": true` on `/health`.
