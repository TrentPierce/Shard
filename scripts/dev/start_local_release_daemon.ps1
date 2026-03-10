param(
    [int]$ControlPort = 9191,
    [int]$TelemetryWsPort = 9193,
    [string]$ModelPath = "",
    [string]$ModelId = "",
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

function Resolve-ModelId {
    param(
        [string]$Path,
        [string]$ExplicitModelId
    )

    if ($ExplicitModelId) {
        return $ExplicitModelId
    }

    $leaf = [System.IO.Path]::GetFileNameWithoutExtension($Path).ToLowerInvariant()
    $full = $Path.ToLowerInvariant()

    if ($full.Contains("qwen")) {
        if ($full.Contains("9b")) {
            return "Qwen/Qwen3.5-9B"
        }
        if ($full.Contains("0.8b")) {
            return "Qwen/Qwen3.5-0.8B"
        }
        if ($full.Contains("0.6b")) {
            return "Qwen/Qwen3-0.6B"
        }
        return "Qwen/Qwen"
    }

    if ($full.Contains("llama-3.2-1b")) {
        return "meta-llama/Llama-3.2-1B"
    }

    if ($full.Contains("tinyllama")) {
        return "TinyLlama/TinyLlama-1.1B-Chat-v1.0"
    }

    return $leaf
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
$resolvedModelId = Resolve-ModelId -Path $ModelPath -ExplicitModelId $ModelId
$psi.Arguments = @(
    "--control-port", $ControlPort,
    "--telemetry-ws-port", $TelemetryWsPort,
    "--model-id", $resolvedModelId,
    "--bootstrap-node", "/ip4/35.175.242.222/tcp/4001/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV",
    "--bootstrap-node", "/ip4/35.175.242.222/udp/9092/quic-v1/p2p/12D3KooWPQqkkZk7NeWA2b1FeWYuBFRW8X7Q9ugymnzxeKJHFLUV",
    "--contribute"
) -join " "

foreach ($envFile in $EnvFiles) {
    Set-EnvFromFile -ProcessInfo $psi -Path $envFile
}

if ($BenchmarkProfile -eq "long") {
    $psi.EnvironmentVariables["SHARD_SCOUT_BOOTSTRAP_ALLOW_HARD_CIRCUIT"] = "true"
}

if ($resolvedModelId.ToLowerInvariant().Contains("qwen")) {
    $psi.EnvironmentVariables["SHARD_SPECULATIVE_STRICT_MODE"] = "true"
}

$psi.EnvironmentVariables["BITNET_LIB"] = $lib
$psi.EnvironmentVariables["BITNET_MODEL"] = $ModelPath
$psi.EnvironmentVariables["PATH"] = "{0};{1}" -f (Split-Path $lib), $psi.EnvironmentVariables["PATH"]

[void][System.Diagnostics.Process]::Start($psi)

Start-Sleep -Seconds 6

$healthUrl = "http://127.0.0.1:{0}/health" -f $ControlPort
Invoke-RestMethod -Uri $healthUrl -Method Get | ConvertTo-Json -Depth 6
