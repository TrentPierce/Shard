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

function Get-EnvMap {
    param([string]$Path)
    $map = @{}
    foreach ($line in Get-Content -Path $Path) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#")) {
            continue
        }
        $parts = $trimmed.Split("=", 2)
        if ($parts.Length -ne 2) {
            continue
        }
        $map[$parts[0].Trim()] = $parts[1].Trim()
    }
    return $map
}

function Assert-LocalRuntimeProfile {
    param(
        [string]$BaseUrl,
        [hashtable]$Expected
    )
    $health = Invoke-RestMethod -Uri "$BaseUrl/health" -Method Get
    $scoutConfig = Invoke-RestMethod -Uri "$BaseUrl/v1/system/scout-config" -Method Get
    $expectedQueueCapValue = if ($Expected.ContainsKey("SHARD_VERIFIER_QUEUE_CAP")) { $Expected["SHARD_VERIFIER_QUEUE_CAP"] } else { "0" }
    $expectedProfileValue = if ($Expected.ContainsKey("SHARD_RELEASE_PROFILE")) { $Expected["SHARD_RELEASE_PROFILE"] } else { "" }
    $expectedLongMinValue = if ($Expected.ContainsKey("SHARD_SCOUT_LONG_REQUEST_MIN_TOKENS")) { $Expected["SHARD_SCOUT_LONG_REQUEST_MIN_TOKENS"] } else { "0" }
    if ($health.status -ne "ok") {
        throw "Local daemon status is '$($health.status)'"
    }
    if (-not $health.ready_for_inference) {
        throw "Local daemon is not ready for inference"
    }
    $expectedQueueCap = [int]$expectedQueueCapValue
    if ($expectedQueueCap -gt 0 -and [int]$health.verifier_queue_cap -ne $expectedQueueCap) {
        throw "Local verifier_queue_cap=$($health.verifier_queue_cap) expected=$expectedQueueCap"
    }
    $expectedProfile = [string]$expectedProfileValue
    if ($expectedProfile -and [string]$scoutConfig.config.profile -ne $expectedProfile) {
        throw "Local profile=$($scoutConfig.config.profile) expected=$expectedProfile"
    }
    $expectedLongMin = [int]$expectedLongMinValue
    if ([int]$scoutConfig.config.speculative.long_request_min_tokens -ne $expectedLongMin) {
        throw "Local long_request_min_tokens=$($scoutConfig.config.speculative.long_request_min_tokens) expected=$expectedLongMin"
    }
    if ($Expected.ContainsKey("SHARD_SCOUT_TIMEOUT_VERIFIER_RATIO_LONG")) {
        $expectedLongRatio = [double]$Expected["SHARD_SCOUT_TIMEOUT_VERIFIER_RATIO_LONG"]
        $actualLongRatio = [double]$scoutConfig.config.speculative.timeout.verifier_ratio_long
        if ([Math]::Abs($actualLongRatio - $expectedLongRatio) -gt 0.000001) {
            throw "Local verifier_ratio_long=$actualLongRatio expected=$expectedLongRatio"
        }
    }
    return @{
        health = $health
        scoutConfig = $scoutConfig
    }
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\\..")).Path
$rustDir = Join-Path $root "desktop\\rust"
$localRunScript = Join-Path $root "scripts\\dev\\run_local_release_daemon.cmd"
$localExe = Join-Path $rustDir "target\\release\\shard-daemon.exe"
$rc1Env = Join-Path $root "deploy\\release\\rc1.env"
$benchmarkEnv = Join-Path $root "deploy\\release\\benchmark.env"
$longBenchmarkEnv = Join-Path $root "deploy\\release\\long_benchmark.env"
$selectedBenchmarkEnv = if ($BenchmarkProfile -eq "long") { $longBenchmarkEnv } else { $benchmarkEnv }
$selectedRemoteBenchmarkEnvName = if ($BenchmarkProfile -eq "long") { "long_benchmark.env" } else { "benchmark.env" }
$expectedProfileMap = Get-EnvMap -Path $selectedBenchmarkEnv

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
cmd /c "taskkill /F /IM shard-daemon.exe >NUL 2>NUL" | Out-Null
cmd /c start "" "$localRunScript" $BenchmarkProfile | Out-Null
Start-Sleep -Seconds 3
$localRuntime = Assert-LocalRuntimeProfile -BaseUrl "http://127.0.0.1:9191" -Expected $expectedProfileMap
Write-Host ("    local status={0} engine_loaded={1} verifier_queue_cap={2} profile={3}" -f `
    $localRuntime.health.status, `
    $localRuntime.health.engine_loaded, `
    $localRuntime.health.verifier_queue_cap, `
    $localRuntime.scoutConfig.config.profile)

$remoteRc1Env = "/tmp/rc1.env"
$remoteBenchmarkEnv = "/tmp/$selectedRemoteBenchmarkEnvName"
scp -i $KeyPath $rc1Env "${Ec2User}@${Ec2Host}:${remoteRc1Env}" | Out-Null
if ($ApplyBenchmarkProfile) {
    scp -i $KeyPath $selectedBenchmarkEnv "${Ec2User}@${Ec2Host}:${remoteBenchmarkEnv}" | Out-Null
}

$benchmarkEnvSetup = if ($ApplyBenchmarkProfile) {
@"
sudo install -m 0644 $remoteBenchmarkEnv /etc/shard/benchmark.env
printf '%s\n' '[Service]' 'EnvironmentFile=/etc/shard/benchmark.env' | sudo tee /etc/systemd/system/shard-daemon.service.d/30-benchmark.conf > /dev/null
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
curl -fsS http://127.0.0.1:9091/health > /tmp/shard-health.json
curl -fsS http://127.0.0.1:9091/v1/system/scout-config > /tmp/shard-scout-config.json
python3 - <<'PY'
import json
import math
from pathlib import Path

health = json.loads(Path("/tmp/shard-health.json").read_text(encoding="utf-8"))
scout_config = json.loads(Path("/tmp/shard-scout-config.json").read_text(encoding="utf-8"))
expected_profile = "$($expectedProfileMap["SHARD_RELEASE_PROFILE"])"
expected_queue_cap = int("$($expectedProfileMap["SHARD_VERIFIER_QUEUE_CAP"])")
expected_long_min = int("$($expectedProfileMap["SHARD_SCOUT_LONG_REQUEST_MIN_TOKENS"])")
expected_long_ratio_raw = "$($expectedProfileMap["SHARD_SCOUT_TIMEOUT_VERIFIER_RATIO_LONG"])"
expected_long_ratio = float(expected_long_ratio_raw) if expected_long_ratio_raw else None

if health.get("status") != "ok":
    raise SystemExit(f"remote status={health.get('status')}")
if not health.get("ready_for_inference"):
    raise SystemExit("remote ready_for_inference=false")
if int(health.get("verifier_queue_cap", 0) or 0) != expected_queue_cap:
    raise SystemExit(
        f"remote verifier_queue_cap={health.get('verifier_queue_cap')} expected={expected_queue_cap}"
    )
config = scout_config.get("config", {})
speculative = config.get("speculative", {})
timeout_cfg = speculative.get("timeout", {})
if expected_profile and config.get("profile") != expected_profile:
    raise SystemExit(
        f"remote profile={config.get('profile')} expected={expected_profile}"
    )
if int(speculative.get("long_request_min_tokens", 0) or 0) != expected_long_min:
    raise SystemExit(
        f"remote long_request_min_tokens={speculative.get('long_request_min_tokens')} expected={expected_long_min}"
    )
if expected_long_ratio is not None:
    actual_long_ratio = timeout_cfg.get("verifier_ratio_long")
    if actual_long_ratio is None:
        raise SystemExit("remote timeout.verifier_ratio_long missing")
    if math.fabs(float(actual_long_ratio) - expected_long_ratio) > 1e-6:
        raise SystemExit(
            f"remote verifier_ratio_long={actual_long_ratio} expected={expected_long_ratio}"
        )

print(
    json.dumps(
        {
            "status": health.get("status"),
            "engine_loaded": health.get("engine_loaded"),
            "verifier_queue_cap": health.get("verifier_queue_cap"),
            "profile": config.get("profile"),
            "long_request_min_tokens": speculative.get("long_request_min_tokens"),
            "verifier_ratio_long": timeout_cfg.get("verifier_ratio_long"),
        }
    )
)
PY
"@

Write-Host "==> Building and deploying EC2 shard-daemon ($Ec2User@$Ec2Host)"
ssh -i $KeyPath "$Ec2User@$Ec2Host" $remote

Write-Host "==> Done"
