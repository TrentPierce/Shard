$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")

Write-Host "[1/3] Building shard-daemon and shard-load-test"
Push-Location (Join-Path $root "desktop/rust")
cargo build --release
Pop-Location

Write-Host "[2/3] Installing web dependencies"
Push-Location (Join-Path $root "web")
npm ci
Pop-Location

Write-Host "[3/3] Quickstart checks"
Write-Host "Run daemon: docker compose up --build shard-daemon"
Write-Host "Open dashboard: http://127.0.0.1:9091/dashboard"
Write-Host "Run load test: .\\desktop\\rust\\target\\release\\shard-load-test.exe --requests 100 --concurrency 100"
