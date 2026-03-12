# Release RC Runbook

This runbook is the operator procedure for running and judging a Shard release candidate.

## 1. Freeze Config and Commit

1. Use one code revision on both verifiers:
   - local verifier
   - EC2 verifier
2. Use one runtime profile on both verifiers:
   - `deploy/release/rc1.env`
   - plus only the minimum overlay required for the scenario you are testing
3. Verify parity on both hosts:

```bash
curl http://127.0.0.1:9091/health
curl http://127.0.0.1:9091/metrics/summary
```

## 2. Apply Frozen Profile

### Local Docker mesh

`deploy/demo/docker-compose.mesh.yml` is the canonical local verifier path and
already loads both:

- `deploy/release/rc1.env`
- one benchmark overlay env selected for the class under test

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

The parity check must report `ok: true` before any release validation.

## 3. Validate the Shipping Product Path

Start with the normal local-first product path:

- browser `Auto` mode for simple prompts
- browser `Auto` mode for clearly complex prompts
- verifier `standard`
- verifier `local_speculative` only as an explicit comparison path
- multi-backend failover with at least one degraded or unavailable backend

```bash
curl http://127.0.0.1:9091/health
curl http://127.0.0.1:9091/metrics/summary
```

The release candidate should not be judged primarily on browser-scout participation. Judge it on:

- whether browser `Auto` chooses sane routes
- whether browser-local answers stay fast and correct
- whether verifier escalation remains healthy and low-error
- whether `standard` remains the correct default
- whether `local_speculative` is stable and worth enabling later
- whether failover and mesh routing behave predictably when backends degrade

## 4. Validate Desktop-Local Speculative Throughput

Run repeated `standard` versus `local_speculative` comparisons on the target hardware class before enabling any production uplift claim.

The release matrix now fails closed by default:

- If verifier readiness never clears, the scenario aborts.
- If inter-run queue drain never clears, the matrix aborts.
- Only use `--allow-dirty-readiness` or `--allow-dirty-flush` for local debugging.

## 5. Optional Experimental WAN Benchmark

Only run the experimental WAN path if you are explicitly benchmarking it. Use [REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md](REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md) and keep the results separate from the product release decision.

## 6. Go/No-Go Decision

Release only if the product path meets the gates in [release-rc-checklist.md](D:\Dev\Projects\Shard\Shard\docs\release-rc-checklist.md):

- browser-local responses are healthy
- verifier-routed responses are healthy
- `standard` is healthy as the default routed path
- contributor-control endpoints remain healthy for API-driven node participation
- failover and mesh routing remain predictable

Experimental WAN data may inform future research, but it should not decide a normal product launch.

## 7. Rollback Conditions

Rollback immediately if any of these persist for 10 minutes:

- `p95_latency_ms > 6000`
- `error_rate > 8%`
- repeated verifier degradation with no recovery
- false-ready health states or route decisions that trap users in failing execution paths

## 8. Rollback Commands

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
