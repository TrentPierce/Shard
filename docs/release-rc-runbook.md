# Release RC Runbook

This runbook is the operator procedure for running and judging a Shard release candidate.

## 1. Freeze Config and Commit

1. Use one code revision on both verifiers:
   - local verifier
   - EC2 verifier
2. Use one runtime profile on both verifiers:
   - `deploy/release/rc1.env`
3. Verify parity on both hosts:

```bash
curl http://127.0.0.1:9091/health
curl http://127.0.0.1:9091/v1/system/scout-config
```

## 2. Apply Frozen Profile

### Local Docker mesh

`deploy/demo/docker-compose.mesh.yml` already loads `deploy/release/rc1.env`.

```bash
docker compose -f deploy/demo/docker-compose.mesh.yml down -v
docker compose -f deploy/demo/docker-compose.mesh.yml up -d --build bootstrap
docker compose -f deploy/demo/docker-compose.mesh.yml up -d --scale shard-node=2 shard-node
```

### EC2 systemd verifier

```bash
sudo mkdir -p /etc/shard
sudo cp deploy/release/rc1.env /etc/shard/rc1.env
sudo chmod 0644 /etc/shard/rc1.env
sudo systemctl edit shard-daemon
```

Add:

```ini
[Service]
EnvironmentFile=/etc/shard/rc1.env
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl restart shard-daemon
sudo systemctl status shard-daemon --no-pager
```

### Optional benchmark overrides (recommended for matrix runs)

To measure verifier/scout behavior without policy-throttling noise, apply
`deploy/release/benchmark.env` on both nodes in addition to `rc1.env`.

```bash
sudo cp deploy/release/benchmark.env /etc/shard/benchmark.env
sudo systemctl edit shard-daemon
```

Add:

```ini
[Service]
EnvironmentFile=/etc/shard/benchmark.env
```

## 3. Run Release Matrix

```bash
python benchmarks/distributed/run_release_matrix.py \
  --one-node-pool http://127.0.0.1:9191 \
  --two-node-pool http://127.0.0.1:9191,http://35.175.242.222:9091 \
  --runs-per-scenario 3 \
  --scouts 24 \
  --rate 4 \
  --duration 60 \
  --scout-workers 4
```

Artifacts are written to `reports/release-rc/release-rc-<timestamp>/`.

## 4. Go/No-Go Decision

Read:

- `go-no-go-summary.json`
- `go-no-go-report.md`

Release only if recommendation is `GO` and all gates in `docs/release-rc-checklist.md` pass.

## 5. Rollback Conditions

Rollback immediately if any of these persist for 10 minutes:

- `p95_latency_ms > 6000`
- `error_rate > 8%`
- repeated scout ingress hard-circuit events and no recovery

## 6. Rollback Commands

### Docker mesh/local verifier

```bash
docker compose -f deploy/demo/docker-compose.mesh.yml down -v
git checkout <last-known-good-tag-or-commit>
docker compose -f deploy/demo/docker-compose.mesh.yml up -d --build bootstrap
docker compose -f deploy/demo/docker-compose.mesh.yml up -d --scale shard-node=2 shard-node
```

### EC2 verifier

```bash
ssh -i <key.pem> ubuntu@<ec2-host>
cd /opt/shard/repo
git checkout <last-known-good-tag-or-commit>
sudo install -m 755 desktop/rust/target/release/shard-daemon /opt/shard/bin/shard-daemon
sudo systemctl restart shard-daemon
curl http://127.0.0.1:9091/health
```
