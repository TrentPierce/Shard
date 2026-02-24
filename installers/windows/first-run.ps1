$ErrorActionPreference = "Stop"

param(
    [string]$InstallDir = "$env:ProgramFiles\ShardNode",
    [int]$ControlPort = 9091,
    [string]$BootstrapHealthUrl = "http://127.0.0.1:9091/v1/system/bootstrap",
    [switch]$DownloadModelIfMissing,
    [switch]$NonInteractive
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
    if (-not $NonInteractive -and -not $DownloadModelIfMissing) {
        $answer = Read-Host "Download baseline model now? (y/N)"
        if ($answer -match "^(y|yes)$") {
            $DownloadModelIfMissing = $true
        }
    }
    if ($DownloadModelIfMissing) {
        $modelUrl = "https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
        Write-Host "Downloading baseline model from $modelUrl ..."
        try {
            Invoke-WebRequest -Uri $modelUrl -OutFile $env:BITNET_MODEL
        } catch {
            Write-Host "Model download failed: $($_.Exception.Message)" -ForegroundColor Yellow
        }
    } else {
        Write-Host "Download a model before contribution or set BITNET_MODEL to an existing path." -ForegroundColor Yellow
    }
}

Write-Host "Checking firewall rule..."
$ruleName = "Shard Node Control Plane"
if (-not (Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue)) {
    $createRule = $true
    if (-not $NonInteractive) {
        $firewallAnswer = Read-Host "Create firewall allow rule for port $ControlPort? (Y/n)"
        if ($firewallAnswer -match "^(n|no)$") {
            $createRule = $false
        }
    }
    if ($createRule) {
        New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Protocol TCP -LocalPort $ControlPort -Action Allow | Out-Null
        Write-Host "Created firewall allow rule for port $ControlPort."
    } else {
        Write-Host "Skipped firewall rule creation by user choice." -ForegroundColor Yellow
    }
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

Write-Host "Checking bootstrap connectivity..."
try {
    $bootstrap = Invoke-RestMethod -Uri $BootstrapHealthUrl -TimeoutSec 10
    if ($bootstrap.local_peer_id) {
        Write-Host "Bootstrap discovery reachable." -ForegroundColor Green
    } else {
        Write-Host "Bootstrap endpoint reachable but response was unexpected." -ForegroundColor Yellow
    }
} catch {
    Write-Host "Bootstrap connectivity check failed: $($_.Exception.Message)" -ForegroundColor Yellow
}
