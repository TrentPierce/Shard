param(
    [string]$Ec2Host = "35.175.242.222",
    [string]$Ec2User = "ubuntu",
    [string]$KeyPath = "E:/Clawdbot.pem",
    [string]$RemoteRepo = "/home/ubuntu/Shard"
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\\..")).Path
$rustDir = Join-Path $root "desktop\\rust"
$localRunScript = Join-Path $root "scripts\\dev\\run_local_daemon.cmd"
$localExe = Join-Path $rustDir "target\\release\\shard-daemon.exe"

Write-Host "==> Building local shard-daemon (release)"
Push-Location $rustDir
$sw = [Diagnostics.Stopwatch]::StartNew()
cargo build -p shard-daemon --release
$sw.Stop()
Pop-Location
Write-Host ("    local build time: {0}" -f $sw.Elapsed)

Write-Host "==> Restarting local shard-daemon"
taskkill /F /IM shard-daemon.exe 2>$null | Out-Null
cmd /c start "" "$localRunScript" | Out-Null
Start-Sleep -Seconds 3
$localHealth = Invoke-RestMethod -Uri "http://127.0.0.1:9091/health" -Method Get
Write-Host ("    local status={0} engine_loaded={1}" -f $localHealth.status, $localHealth.engine_loaded)

$remote = @"
set -euo pipefail
source \$HOME/.cargo/env
cd $RemoteRepo
git pull --ff-only origin main
cd desktop/rust
/usr/bin/time -f 'remote_build_time=%E' cargo build -p shard-daemon --release
sudo install -m 755 target/release/shard-daemon /opt/shard/bin/shard-daemon
sudo systemctl restart shard-daemon
sleep 2
systemctl is-active shard-daemon
curl -fsS http://127.0.0.1:9091/health | head -c 220
"@

Write-Host "==> Building and deploying EC2 shard-daemon ($Ec2User@$Ec2Host)"
ssh -i $KeyPath "$Ec2User@$Ec2Host" $remote

Write-Host "==> Done"
