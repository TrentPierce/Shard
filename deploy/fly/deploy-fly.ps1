param(
    [string]$AppName = "shard-fly-bench",
    [string[]]$Regions = @("iad", "lax", "lhr"),
    [int]$VolumeSizeGb = 20,
    [string]$BootstrapNode = ""
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command flyctl -ErrorAction SilentlyContinue)) {
    throw "flyctl is required. Install it first."
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$configPath = Join-Path $root "fly.toml"

if (-not (Test-Path $configPath)) {
    throw "Missing fly.toml at $configPath"
}

try {
    flyctl apps show -a $AppName | Out-Null
} catch {
    flyctl apps create $AppName | Out-Null
}

if ($BootstrapNode) {
    flyctl secrets set SHARD_DEFAULT_BOOTSTRAP="$BootstrapNode" -a $AppName | Out-Null
}

foreach ($region in $Regions) {
    try {
        flyctl volumes create shard_data --region $region --size $VolumeSizeGb -a $AppName -y | Out-Null
    } catch {
        Write-Host "volume create skipped for $region: $($_.Exception.Message)"
    }
}

$primaryRegion = $Regions[0]
flyctl deploy --config $configPath --app $AppName --remote-only --region $primaryRegion

$machineJson = flyctl machine list -a $AppName --json
$machines = $machineJson | ConvertFrom-Json
if (-not $machines -or $machines.Count -lt 1) {
    throw "No machines found after deploy."
}

$sourceMachineId = $machines[0].id
foreach ($region in $Regions | Select-Object -Skip 1) {
    try {
        flyctl machine clone $sourceMachineId --app $AppName --region $region -y | Out-Null
    } catch {
        Write-Host "machine clone skipped for $region: $($_.Exception.Message)"
    }
}

flyctl status -a $AppName
