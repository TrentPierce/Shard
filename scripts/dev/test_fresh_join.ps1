$ErrorActionPreference = "Stop"

$tempRoot = Join-Path $env:TEMP "shard-fresh-join-0605"
if (Test-Path $tempRoot) {
    Remove-Item $tempRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $tempRoot | Out-Null

$daemon = "D:\Dev\Projects\Shard\Shard\desktop\rust\target\release\shard-daemon.exe"
$log = Join-Path $tempRoot "daemon.log"
$err = Join-Path $tempRoot "daemon.err"

$previousShardDataDir = $env:SHARD_DATA_DIR
$previousBitnetModel = $env:BITNET_MODEL
$previousBitnetLib = $env:BITNET_LIB

$env:SHARD_DATA_DIR = $tempRoot
$env:BITNET_MODEL = "D:\Dev\Projects\Shard\Shard\models\Llama-3.2-1B-Instruct-Q4_K_M.gguf"
$env:BITNET_LIB = "D:\Dev\Projects\Shard\Shard\desktop\rust\target\release\shard_engine.dll"

$proc = Start-Process `
    -FilePath $daemon `
    -ArgumentList @(
        "--control-port", "19191",
        "--tcp-port", "14001",
        "--webrtc-port", "19090",
        "--quic-port", "19092",
        "--telemetry-ws-port", "19093",
        "--public-api",
        "--public-host", "127.0.0.1"
    ) `
    -RedirectStandardOutput $log `
    -RedirectStandardError $err `
    -PassThru `
    -WorkingDirectory "D:\Dev\Projects\Shard\Shard" `
    -WindowStyle Hidden

try {
    Start-Sleep -Seconds 15
    $health = curl.exe -s http://127.0.0.1:19191/health
    Write-Output $health
}
finally {
    if ($proc -and -not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force
    }
    Start-Sleep -Seconds 1
    $env:SHARD_DATA_DIR = $previousShardDataDir
    $env:BITNET_MODEL = $previousBitnetModel
    $env:BITNET_LIB = $previousBitnetLib
    if (Test-Path $log) {
        Write-Output "--- daemon.log ---"
        Get-Content $log -Tail 20
    }
    if (Test-Path $err) {
        Write-Output "--- daemon.err ---"
        Get-Content $err -Tail 20
    }
}
