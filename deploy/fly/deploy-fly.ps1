param(
    [string]$AppName = "shard-fly-bench",
    [string[]]$Regions = @("iad", "lax", "lhr"),
    [int]$VolumeSizeGb = 20,
    [string]$BootstrapNode = ""
)

$ErrorActionPreference = "Stop"
$Regions = @($Regions | ForEach-Object { $_ -split "," } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

function Assert-FlySuccess {
    param([string]$Message)
    if ($LASTEXITCODE -ne 0) {
        throw $Message
    }
}

function Get-FlyMachines {
    param([string]$App)
    $machineJson = flyctl machine list -a $App --json
    Assert-FlySuccess "Failed to list Fly machines for $App"
    @($machineJson | ConvertFrom-Json)
}

function Get-FlyMachineControlHealth {
    param([object]$Machine)
    $check = $Machine.checks | Where-Object { $_.name -eq "control" } | Select-Object -First 1
    if (-not $check -or [string]::IsNullOrWhiteSpace($check.output)) {
        return $null
    }
    try {
        return ($check.output | ConvertFrom-Json)
    } catch {
        return $null
    }
}

function Wait-FlyMachinesReady {
    param(
        [string]$App,
        [int]$TimeoutSeconds = 240
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $machines = Get-FlyMachines -App $App | Where-Object { $_.state -eq "started" }
        if ($machines.Count -lt 1) {
            Start-Sleep -Seconds 5
            continue
        }

        $peerStates = @()
        $allReady = $true
        foreach ($machine in $machines) {
            $health = Get-FlyMachineControlHealth -Machine $machine
            if (-not $health -or -not $health.ready_for_inference -or [string]::IsNullOrWhiteSpace($health.peer_id)) {
                $allReady = $false
                break
            }
            $peerStates += [pscustomobject]@{
                id              = $machine.id
                region          = $machine.region
                private_ip      = $machine.private_ip
                peer_id         = $health.peer_id
                connected_peers = [int]$health.connected_peers
                known_peers     = [int]$health.known_peers
            }
        }

        if ($allReady -and $peerStates.Count -eq $machines.Count) {
            return $peerStates
        }

        Start-Sleep -Seconds 5
    }

    throw "Timed out waiting for Fly machines to become inference-ready."
}

function Build-FlyBootstrapList {
    param(
        [string]$SeedBootstrap,
        [object[]]$PeerStates
    )

    $bootstrap = New-Object System.Collections.Generic.List[string]
    if (-not [string]::IsNullOrWhiteSpace($SeedBootstrap)) {
        $bootstrap.Add($SeedBootstrap)
    }
    foreach ($peer in $PeerStates) {
        if ([string]::IsNullOrWhiteSpace($peer.private_ip) -or [string]::IsNullOrWhiteSpace($peer.peer_id)) {
            continue
        }
        $bootstrap.Add("/ip6/$($peer.private_ip)/tcp/4001/p2p/$($peer.peer_id)")
        $bootstrap.Add("/ip6/$($peer.private_ip)/udp/9092/quic-v1/p2p/$($peer.peer_id)")
    }

    ($bootstrap | Select-Object -Unique) -join ","
}

function Restart-FlyMachines {
    param(
        [string]$App,
        [object[]]$PeerStates
    )
    foreach ($peer in $PeerStates) {
        flyctl machine restart $peer.id -a $App --force --skip-health-checks | Out-Null
        Assert-FlySuccess "Failed to restart Fly machine $($peer.id)"
    }
}

function Wait-FlyMesh {
    param(
        [string]$App,
        [int]$MinimumConnectedPeers,
        [int]$TimeoutSeconds = 240
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $peerStates = Wait-FlyMachinesReady -App $App -TimeoutSeconds 60
        $meshReady = $peerStates.Count -gt 0 -and (($peerStates | Where-Object {
                    $_.connected_peers -ge $MinimumConnectedPeers
                }).Count -eq $peerStates.Count)
        if ($meshReady) {
            return $peerStates
        }
        Start-Sleep -Seconds 5
    }

    throw "Timed out waiting for Fly mesh to reach connected_peers >= $MinimumConnectedPeers on every machine."
}

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
    Assert-FlySuccess "Failed to create Fly app $AppName"
}

if ($BootstrapNode) {
    flyctl secrets set SHARD_DEFAULT_BOOTSTRAP="$BootstrapNode" -a $AppName --stage | Out-Null
    Assert-FlySuccess "Failed to stage SHARD_DEFAULT_BOOTSTRAP for $AppName"
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
Assert-FlySuccess "Fly deploy failed for $AppName"

$machines = Get-FlyMachines -App $AppName
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

$peerStates = Wait-FlyMachinesReady -App $AppName
if ($peerStates.Count -gt 1) {
    $expandedBootstrap = Build-FlyBootstrapList -SeedBootstrap $BootstrapNode -PeerStates $peerStates
    if (-not [string]::IsNullOrWhiteSpace($expandedBootstrap)) {
        flyctl secrets set SHARD_DEFAULT_BOOTSTRAP="$expandedBootstrap" -a $AppName | Out-Null
        Assert-FlySuccess "Failed to apply expanded Fly bootstrap list for $AppName"
        Restart-FlyMachines -App $AppName -PeerStates $peerStates
        $peerStates = Wait-FlyMesh -App $AppName -MinimumConnectedPeers 2
        $peerStates | Format-Table id, region, peer_id, private_ip, connected_peers, known_peers -AutoSize
    }
}

flyctl status -a $AppName
