$ErrorActionPreference = "Stop"

param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

if (-not (Test-Path $InstallerPath)) {
    throw "Installer not found: $InstallerPath"
}

$signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $signtool) {
    throw "signtool.exe is required to sign the installer."
}

Write-Host "Signing installer: $InstallerPath"
& $signtool.Source sign /fd SHA256 /tr $TimestampUrl /td SHA256 /a $InstallerPath
if ($LASTEXITCODE -ne 0) {
    throw "signtool sign failed with exit code $LASTEXITCODE"
}

Write-Host "Verifying Authenticode signature..."
& $signtool.Source verify /pa /v $InstallerPath
if ($LASTEXITCODE -ne 0) {
    throw "signtool verify failed with exit code $LASTEXITCODE"
}

Write-Host "Signing completed successfully."

