param(
    [int]$ControlPort = 9191,
    [int]$TelemetryWsPort = 9193,
    [string]$ModelPath = "",
    [ValidateSet("short","long","custom")]
    [string]$BenchmarkProfile = "short",
    [string[]]$EnvFiles = @(
        "deploy/release/rc1.env",
        "deploy/release/benchmark.env"
    )
)

$ErrorActionPreference = "Stop"

$defaultEnvFiles = switch ($BenchmarkProfile) {
    "short" { @("deploy/release/rc1.env", "deploy/release/benchmark.env") }
    "long" { @("deploy/release/rc1.env", "deploy/release/long_benchmark.env") }
    default { $EnvFiles }
}

if ($BenchmarkProfile -ne "custom") {
    $EnvFiles = $defaultEnvFiles
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$exe = Join-Path $root "desktop\rust\target\release\shard-daemon.exe"
if (-not (Test-Path $exe)) {
    $exe = Join-Path $root "desktop\rust\target\release\shard-daemon.locked.exe"
}
$lib = Join-Path $root "desktop\rust\target\release\shard_engine.dll"
if (-not (Test-Path $lib)) {
    $lib = Join-Path $root "web\src-tauri\resources\shard_engine.dll"
}

if (-not $ModelPath) {
    $ModelPath = Join-Path $root "models\Llama-3.2-1B-Instruct-Q4_K_M.gguf"
}

if (-not (Test-Path $exe)) {
    throw "Missing daemon binary: $exe"
}
if (-not (Test-Path $lib)) {
    throw "Missing engine DLL: $lib"
}
if (-not (Test-Path $ModelPath)) {
    throw "Missing model file: $ModelPath"
}

function Set-EnvFromFile {
    param(
        [System.Diagnostics.ProcessStartInfo]$ProcessInfo,
        [string]$Path
    )

    $fullPath = if ([System.IO.Path]::IsPathRooted($Path)) {
        $Path
    } else {
        Join-Path $root $Path
    }

    if (-not (Test-Path $fullPath)) {
        throw "Missing env file: $fullPath"
    }

    foreach ($line in Get-Content $fullPath) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#")) {
            continue
        }
        $parts = $trimmed.Split("=", 2)
        if ($parts.Count -ne 2) {
            continue
        }
        $ProcessInfo.EnvironmentVariables[$parts[0]] = $parts[1]
    }
}

Get-Process -Name "shard-daemon","shard-daemon.locked" -ErrorAction SilentlyContinue | Stop-Process -Force

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exe
$psi.WorkingDirectory = (Split-Path $exe)
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$psi.Arguments = @(
    "--control-port", $ControlPort,
    "--telemetry-ws-port", $TelemetryWsPort,
    "--bootstrap-node", "/dns4/35.175.242.222.nip.io/tcp/4001/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV",
    "--bootstrap-node", "/dns4/35.175.242.222.nip.io/udp/9092/quic-v1/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV",
    "--contribute"
) -join " "

foreach ($envFile in $EnvFiles) {
    Set-EnvFromFile -ProcessInfo $psi -Path $envFile
}

$psi.EnvironmentVariables["BITNET_LIB"] = $lib
$psi.EnvironmentVariables["BITNET_MODEL"] = $ModelPath
$psi.EnvironmentVariables["PATH"] = "{0};{1}" -f (Split-Path $lib), $psi.EnvironmentVariables["PATH"]

[void][System.Diagnostics.Process]::Start($psi)

Start-Sleep -Seconds 6

$healthUrl = "http://127.0.0.1:{0}/health" -f $ControlPort
Invoke-RestMethod -Uri $healthUrl -Method Get | ConvertTo-Json -Depth 6
