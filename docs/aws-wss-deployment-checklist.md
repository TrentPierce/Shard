# AWS EC2 Production Checklist (Rust sidecar + `wss://`)

Use this when deploying the Shard Rust `libp2p`/WebSocket sidecar behind Nginx TLS.

## 1) Build and copy the Rust binary

From your build machine:

```bash
cd desktop/rust
cargo build --release
scp target/release/shard-daemon ubuntu@<EC2_PUBLIC_IP>:/tmp/shard-daemon
```

## 2) Run bootstrap script on EC2

```bash
ssh ubuntu@<EC2_PUBLIC_IP>
sudo -i
cd /workspace/Shard
DOMAIN=swarm.example.com \
EMAIL=ops@example.com \
SHARD_TCP_PORT=4001 \
SHARD_WEBRTC_PORT=9090 \
SHARD_QUIC_PORT=9092 \
SHARD_CONTROL_PORT=9091 \
SHARD_TELEMETRY_PORT=9093 \
./scripts/ec2-production-bootstrap.sh
```

## 3) Install daemon binary and start service

```bash
sudo install -o shard -g shard -m 0755 /tmp/shard-daemon /opt/shard/bin/shard-daemon
sudo systemctl restart shard-daemon
sudo systemctl status shard-daemon --no-pager
```

## 4) Verify TLS WebSocket from browser/app

- Frontend URL should use: `wss://swarm.example.com/telemetry/ws`
- Rust daemon continues to listen internally on `0.0.0.0:<PORT>` (default `9093`) and Nginx terminates TLS.

Quick check:

```bash
curl -I https://swarm.example.com/healthz
```

## 5) Security group rules (must be configured in AWS)

Open inbound:

- `80/tcp` (Let's Encrypt HTTP challenge + redirect)
- `443/tcp` (HTTPS/WSS)
- `4001/tcp` (`libp2p` TCP)
- `4101/tcp` (`libp2p` WebSocket transport, default `tcp_port + 100`)
- `9090/udp` (`webrtc-direct`)
- `9092/udp` (QUIC)

## 6) Firewall rules on instance (UFW)

The bootstrap script enables:

- `80/tcp`, `443/tcp`
- `4001/tcp`, `4101/tcp`
- `9090/udp`, `9092/udp`

## 7) Logs and ongoing checks

```bash
sudo journalctl -u shard-daemon -f
sudo nginx -t
sudo systemctl status nginx --no-pager
```
