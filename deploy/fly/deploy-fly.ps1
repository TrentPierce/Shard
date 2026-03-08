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

flyctl status -a $AppName | Out-Null
if ($LASTEXITCODE -ne 0) {
    flyctl apps create $AppName -o personal -y | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create Fly app $AppName"
    }
}

if ($BootstrapNode) {
    flyctl secrets set SHARD_DEFAULT_BOOTSTRAP="$BootstrapNode" -a $AppName --stage | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to set SHARD_DEFAULT_BOOTSTRAP for $AppName"
    }
}

foreach ($region in $Regions) {
    try {
        flyctl volumes create shard_data --region $region --size $VolumeSizeGb -a $AppName -y | Out-Null
    } catch {
        Write-Host ("volume create skipped for {0}: {1}" -f $region, $_.Exception.Message)
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host ("volume create skipped for {0}: exit {1}" -f $region, $LASTEXITCODE)
    }
}

$primaryRegion = $Regions[0]
flyctl deploy --config $configPath --app $AppName --remote-only --primary-region $primaryRegion -y
if ($LASTEXITCODE -ne 0) {
    throw "Fly deploy failed for $AppName"
}

$machineJson = flyctl machine list -a $AppName --json
$machines = $machineJson | ConvertFrom-Json
if (-not $machines -or $machines.Count -lt 1) {
    throw "No machines found after deploy."
}

$sourceMachineId = $machines[0].id
foreach ($region in $Regions | Select-Object -Skip 1) {
    try {
        flyctl machine clone $sourceMachineId --app $AppName --region $region | Out-Null
    } catch {
        Write-Host ("machine clone skipped for {0}: {1}" -f $region, $_.Exception.Message)
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host ("machine clone skipped for {0}: exit {1}" -f $region, $LASTEXITCODE)
    }
}

flyctl status -a $AppName
