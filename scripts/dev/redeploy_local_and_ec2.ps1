param(
    [string]$Ec2Host = "35.175.242.222",
    [string]$Ec2User = "ubuntu",
    [string]$KeyPath = "E:/Clawdbot.pem",
    [string]$RemoteRepo = "/home/ubuntu/Shard",
    [switch]$ApplyBenchmarkProfile = $true,
    [ValidateSet("short","long")]
    [string]$BenchmarkProfile = "short"
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\\..")).Path
$rustDir = Join-Path $root "desktop\\rust"
$localRunScript = Join-Path $root "scripts\\dev\\run_local_daemon.cmd"
$localExe = Join-Path $rustDir "target\\release\\shard-daemon.exe"
$rc1Env = Join-Path $root "deploy\\release\\rc1.env"
$benchmarkEnv = Join-Path $root "deploy\\release\\benchmark.env"
$longBenchmarkEnv = Join-Path $root "deploy\\release\\long_benchmark.env"
$selectedBenchmarkEnv = if ($BenchmarkProfile -eq "long") { $longBenchmarkEnv } else { $benchmarkEnv }
$selectedRemoteBenchmarkEnvName = if ($BenchmarkProfile -eq "long") { "long_benchmark.env" } else { "benchmark.env" }

if (-not (Test-Path $rc1Env)) {
    throw "Missing runtime profile: $rc1Env"
}
if ($ApplyBenchmarkProfile -and -not (Test-Path $selectedBenchmarkEnv)) {
    throw "Missing benchmark profile: $selectedBenchmarkEnv"
}

Write-Host "==> Building local shard-daemon (release)"
Push-Location $rustDir
$sw = [Diagnostics.Stopwatch]::StartNew()
cargo build -p shard-daemon --release
$sw.Stop()
Pop-Location
Write-Host ("    local build time: {0}" -f $sw.Elapsed)

Write-Host "==> Restarting local shard-daemon"
taskkill /F /IM shard-daemon.exe 2>$null | Out-Null
cmd /c start "" "$localRunScript" $BenchmarkProfile | Out-Null
Start-Sleep -Seconds 3
$localHealth = Invoke-RestMethod -Uri "http://127.0.0.1:9091/health" -Method Get
Write-Host ("    local status={0} engine_loaded={1}" -f $localHealth.status, $localHealth.engine_loaded)

$remoteRc1Env = "/tmp/rc1.env"
$remoteBenchmarkEnv = "/tmp/$selectedRemoteBenchmarkEnvName"
scp -i $KeyPath $rc1Env "${Ec2User}@${Ec2Host}:${remoteRc1Env}" | Out-Null
if ($ApplyBenchmarkProfile) {
    scp -i $KeyPath $selectedBenchmarkEnv "${Ec2User}@${Ec2Host}:${remoteBenchmarkEnv}" | Out-Null
}

$benchmarkEnvSetup = if ($ApplyBenchmarkProfile) {
@"
sudo install -m 0644 $remoteBenchmarkEnv /etc/shard/benchmark.env
sudo tee /etc/systemd/system/shard-daemon.service.d/30-benchmark.conf > /dev/null <<'EOF'
[Service]
EnvironmentFile=/etc/shard/benchmark.env
EOF
"@
} else {
@"
sudo rm -f /etc/systemd/system/shard-daemon.service.d/30-benchmark.conf
sudo rm -f /etc/shard/benchmark.env
"@
}

$remote = @"
set -euo pipefail
source \$HOME/.cargo/env
cd $RemoteRepo
git pull --ff-only origin main
cd desktop/rust
/usr/bin/time -f 'remote_build_time=%E' cargo build -p shard-daemon --release
sudo install -m 755 target/release/shard-daemon /opt/shard/bin/shard-daemon
sudo mkdir -p /etc/shard /etc/systemd/system/shard-daemon.service.d
sudo install -m 0644 $remoteRc1Env /etc/shard/rc1.env
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
sudo tee /etc/systemd/system/shard-daemon.service.d/10-model.conf > /dev/null <<'EOF'
[Service]
Environment=BITNET_MODEL=/opt/shard/models/Llama-3.2-1B-Instruct-Q4_K_M.gguf
Environment=BITNET_LIB=/opt/shard/lib/libshard_engine.so
Environment=LD_LIBRARY_PATH=/opt/shard/lib
Environment=RUST_LOG=info
EOF
sudo tee /etc/systemd/system/shard-daemon.service.d/20-rc1.conf > /dev/null <<'EOF'
[Service]
EnvironmentFile=/etc/shard/rc1.env
EOF
$benchmarkEnvSetup
sudo systemctl daemon-reload
sudo systemctl restart shard-daemon
sleep 2
systemctl is-active shard-daemon
curl -fsS http://127.0.0.1:9091/health | head -c 220
echo
curl -fsS http://127.0.0.1:9091/v1/system/scout-config | head -c 320
"@

Write-Host "==> Building and deploying EC2 shard-daemon ($Ec2User@$Ec2Host)"
ssh -i $KeyPath "$Ec2User@$Ec2Host" $remote

Write-Host "==> Done"
