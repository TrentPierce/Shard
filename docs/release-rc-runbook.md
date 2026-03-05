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

`deploy/demo/docker-compose.mesh.yml` is the canonical local verifier path and
already loads both:

- `deploy/release/rc1.env`
- `deploy/release/benchmark.env`

```bash
docker compose -f deploy/demo/docker-compose.mesh.yml down -v
pwsh -File deploy/demo/mesh-up.ps1 -Nodes 2
```

### EC2 systemd verifier

```bash
sudo mkdir -p /etc/shard
sudo cp deploy/release/rc1.env /etc/shard/rc1.env
sudo chmod 0644 /etc/shard/rc1.env
sudo cp deploy/release/benchmark.env /etc/shard/benchmark.env
sudo chmod 0644 /etc/shard/benchmark.env
sudo mkdir -p /etc/systemd/system/shard-daemon.service.d
sudo tee /etc/systemd/system/shard-daemon.service.d/10-model.conf >/dev/null <<'EOF'
[Service]
Environment=BITNET_MODEL=/opt/shard/models/Llama-3.2-1B-Instruct-Q4_K_M.gguf
Environment=BITNET_LIB=/opt/shard/lib/libshard_engine.so
Environment=LD_LIBRARY_PATH=/opt/shard/lib
Environment=RUST_LOG=info
EOF
sudo tee /etc/systemd/system/shard-daemon.service.d/20-rc1.conf >/dev/null <<'EOF'
[Service]
EnvironmentFile=/etc/shard/rc1.env
EOF
sudo tee /etc/systemd/system/shard-daemon.service.d/30-benchmark.conf >/dev/null <<'EOF'
[Service]
EnvironmentFile=/etc/shard/benchmark.env
EOF
sudo rm -f \
  /etc/systemd/system/shard-daemon.service.d/20-scout-runtime.conf \
  /etc/systemd/system/shard-daemon.service.d/20-scout-timeout.conf \
  /etc/systemd/system/shard-daemon.service.d/30-debug-temp.conf \
  /etc/systemd/system/shard-daemon.service.d/40-scout-timeout-fast.conf \
  /etc/systemd/system/shard-daemon.service.d/50-model-llama.conf \
  /etc/systemd/system/shard-daemon.service.d/70-benchmark-rate-limit.conf \
  /etc/systemd/system/shard-daemon.service.d/99-runtime-debug.conf \
  /etc/systemd/system/shard-daemon.service.d/override.conf \
  /etc/systemd/system/shard-daemon.service.d/zz-benchmark-env.conf \
  /etc/systemd/system/shard-daemon.service.d/zz-model-llama.conf \
  /etc/systemd/system/shard-daemon.service.d/zz-scout-timeout-fast.conf
sudo systemctl daemon-reload
sudo systemctl restart shard-daemon
sudo systemctl status shard-daemon --no-pager
```

### Parity check

After both verifiers are up, compare runtime-sensitive fields directly:

```bash
pwsh -File scripts/dev/check_verifier_parity.ps1 \
  -LocalUrl http://127.0.0.1:19091 \
  -RemoteUrl http://35.175.242.222:9091
```

The parity check must report `ok: true` before matrix runs.

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

The release matrix now fails closed by default:

- If verifier readiness never clears, the scenario aborts.
- If inter-run queue drain never clears, the matrix aborts.
- Only use `--allow-dirty-readiness` or `--allow-dirty-flush` for local debugging.

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
