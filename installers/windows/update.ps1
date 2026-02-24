$ErrorActionPreference = "Stop"

param(
    [ValidateSet("stable", "canary")]
    [string]$Channel = "stable",
    [string]$InstallDir = "$env:ProgramFiles\ShardNode",
    [string]$ChannelsFile = "$PSScriptRoot\update-channels.json"
)

function Get-InstalledVersion {
    $version = (Get-ItemProperty -Path "HKLM:\Software\Shard" -Name "Version" -ErrorAction SilentlyContinue).Version
    if (-not $version) {
        $version = (Get-ItemProperty -Path "HKCU:\Software\Shard" -Name "Version" -ErrorAction SilentlyContinue).Version
    }
    if (-not $version) {
        return "0.0.0"
    }
    return $version
}

function Compare-Version($a, $b) {
    $aParts = $a.Split('.') | ForEach-Object {[int]$_}
    $bParts = $b.Split('.') | ForEach-Object {[int]$_}
    for ($i = 0; $i -lt 3; $i++) {
        if ($aParts[$i] -gt $bParts[$i]) { return 1 }
        if ($aParts[$i] -lt $bParts[$i]) { return -1 }
    }
    return 0
}

if (-not (Test-Path $ChannelsFile)) {
    throw "Channel config not found: $ChannelsFile"
}

$channels = Get-Content -Raw $ChannelsFile | ConvertFrom-Json
$manifestUrl = $channels.$Channel.manifest_url
if (-not $manifestUrl) {
    throw "No manifest URL configured for channel $Channel"
}

Write-Host "Checking update channel '$Channel' at $manifestUrl"
$manifest = Invoke-RestMethod -Uri $manifestUrl -TimeoutSec 20

if (-not $manifest.version -or -not $manifest.installer_url -or -not $manifest.sha256) {
    throw "Manifest missing required fields: version, installer_url, sha256"
}

$installed = Get-InstalledVersion
if ((Compare-Version $manifest.version $installed) -le 0) {
    Write-Host "No update available. Installed=$installed, Channel=$($manifest.version)"
    exit 0
}

Write-Host "Updating from $installed to $($manifest.version)"
$tempInstaller = Join-Path $env:TEMP "shard-update-$($manifest.version).exe"
Invoke-WebRequest -Uri $manifest.installer_url -OutFile $tempInstaller

$downloadHash = (Get-FileHash -Path $tempInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
if ($downloadHash -ne $manifest.sha256.ToLowerInvariant()) {
    throw "Installer hash mismatch. Expected $($manifest.sha256), got $downloadHash"
}

$rollbackRoot = Join-Path $env:ProgramData "Shard\rollback"
$rollbackDir = Join-Path $rollbackRoot ("preupdate-" + (Get-Date -Format "yyyyMMddHHmmss"))
New-Item -ItemType Directory -Path $rollbackDir -Force | Out-Null

if (Test-Path $InstallDir) {
    Write-Host "Backing up existing install to $rollbackDir"
    Copy-Item -Path (Join-Path $InstallDir "*") -Destination $rollbackDir -Recurse -Force
}

$installerArgs = "/S /NOONBOARD"
$process = Start-Process -FilePath $tempInstaller -ArgumentList $installerArgs -Wait -PassThru
if ($process.ExitCode -ne 0) {
    Write-Warning "Update failed (exit $($process.ExitCode)); restoring rollback backup."
    if (Test-Path $InstallDir) {
        Remove-Item -Path (Join-Path $InstallDir "*") -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    Copy-Item -Path (Join-Path $rollbackDir "*") -Destination $InstallDir -Recurse -Force
    throw "Update failed and rollback restored."
}

Write-Host "Update completed successfully to $($manifest.version)."

