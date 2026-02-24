$ErrorActionPreference = "Stop"

param(
    [string]$InstallDir = "$env:ProgramFiles\ShardNode",
    [int]$ControlPort = 9091
)

Write-Host "Shard First-Run Onboarding" -ForegroundColor Cyan

$dataDir = Join-Path $env:ProgramData "Shard"
$identityDir = Join-Path $dataDir "identity"
$modelDir = Join-Path $dataDir "models"
New-Item -ItemType Directory -Path $identityDir -Force | Out-Null
New-Item -ItemType Directory -Path $modelDir -Force | Out-Null

if (-not $env:BITNET_MODEL) {
    $env:BITNET_MODEL = Join-Path $modelDir "model.gguf"
}

if (-not (Test-Path $env:BITNET_MODEL)) {
    Write-Host "Model not found at $($env:BITNET_MODEL)." -ForegroundColor Yellow
    Write-Host "Download a model before contribution or set BITNET_MODEL to an existing path." -ForegroundColor Yellow
}

Write-Host "Checking firewall rule..."
$ruleName = "Shard Node Control Plane"
if (-not (Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Protocol TCP -LocalPort $ControlPort -Action Allow | Out-Null
    Write-Host "Created firewall allow rule for port $ControlPort."
}

Write-Host "Starting service health check..."
Start-Sleep -Seconds 2
try {
    $response = Invoke-RestMethod -Uri "http://127.0.0.1:$ControlPort/health" -TimeoutSec 10
    if ($response.status -eq "ok") {
        Write-Host "Health check passed. Node is ready." -ForegroundColor Green
    } else {
        Write-Host "Health check returned unexpected response." -ForegroundColor Yellow
    }
} catch {
    Write-Host "Health check failed: $($_.Exception.Message)" -ForegroundColor Red
}

