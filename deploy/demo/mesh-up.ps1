param(
    [int]$Nodes = 2,
    [string]$ComposeFile = "deploy/demo/docker-compose.mesh.yml",
    [int]$MaxWaitSeconds = 120
)

$ErrorActionPreference = "Stop"

Write-Host "Starting bootstrap node..."
docker compose -f $ComposeFile up -d --build bootstrap | Out-Null

$deadline = (Get-Date).AddSeconds($MaxWaitSeconds)
$peerId = $null
while ((Get-Date) -lt $deadline) {
    try {
        $health = Invoke-RestMethod -Uri "http://localhost:19091/health" -TimeoutSec 3
        if ($health.peer_id) {
            $peerId = [string]$health.peer_id
            break
        }
    } catch {
        Start-Sleep -Seconds 2
    }
}

if (-not $peerId) {
    throw "Bootstrap node did not report peer_id within $MaxWaitSeconds seconds."
}

$bootstrapAddr = "/dns4/bootstrap/tcp/4001/p2p/$peerId"
$env:SHARD_BOOTSTRAP_NODE = $bootstrapAddr

Write-Host "Bootstrap peer: $bootstrapAddr"
Write-Host "Starting shard-node scale=$Nodes ..."
docker compose -f $ComposeFile up -d --scale shard-node=$Nodes shard-node | Out-Null

Write-Host ""
Write-Host "Mesh started."
Write-Host "Gateway health:  http://localhost:19091/health"
Write-Host "Gateway metrics: http://localhost:19091/metrics/summary"
Write-Host "To stop: docker compose -f $ComposeFile down -v"
