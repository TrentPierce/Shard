param(
    [string]$LocalUrl = "http://127.0.0.1:19091",
    [string]$RemoteUrl = "http://35.175.242.222:9091",
    [int]$TimeoutSec = 30
)

$ErrorActionPreference = "Stop"

function Get-Json($url) {
    $payload = & curl.exe -sS --max-time $TimeoutSec $url
    if ($LASTEXITCODE -ne 0) {
        throw "curl failed for $url"
    }
    $payload | ConvertFrom-Json
}

$localHealth = Get-Json "$LocalUrl/health"
$remoteHealth = Get-Json "$RemoteUrl/health"
$localScoutConfig = Get-Json "$LocalUrl/v1/system/scout-config"
$remoteScoutConfig = Get-Json "$RemoteUrl/v1/system/scout-config"

$summary = [ordered]@{
    local = [ordered]@{
        url = $LocalUrl
        rust_version = $localHealth.rust_version
        model_id = $localHealth.model_id
        bitnet_model = $localHealth.bitnet_model
        readiness_reason = $localHealth.readiness_reason
        race_timeout_ms = $localHealth.race_timeout_ms
        scout_profile = $localScoutConfig.config.profile
        release_profile = $localScoutConfig.config.profile
        backpressure_start_queue_depth = $localScoutConfig.config.backpressure.start_queue_depth
        admission_queue_depth_soft = $localScoutConfig.config.admission.queue_depth_soft
        admission_queue_depth_hard = $localScoutConfig.config.admission.queue_depth_hard
        active_cap_base = $localScoutConfig.config.active_cap.base
        active_cap_soft = $localScoutConfig.config.active_cap.soft
        active_cap_hard = $localScoutConfig.config.active_cap.hard
        verifier_queue_cap = $localHealth.verifier_queue_cap
    }
    remote = [ordered]@{
        url = $RemoteUrl
        rust_version = $remoteHealth.rust_version
        model_id = $remoteHealth.model_id
        bitnet_model = $remoteHealth.bitnet_model
        readiness_reason = $remoteHealth.readiness_reason
        race_timeout_ms = $remoteHealth.race_timeout_ms
        scout_profile = $remoteScoutConfig.config.profile
        release_profile = $remoteScoutConfig.config.profile
        backpressure_start_queue_depth = $remoteScoutConfig.config.backpressure.start_queue_depth
        admission_queue_depth_soft = $remoteScoutConfig.config.admission.queue_depth_soft
        admission_queue_depth_hard = $remoteScoutConfig.config.admission.queue_depth_hard
        active_cap_base = $remoteScoutConfig.config.active_cap.base
        active_cap_soft = $remoteScoutConfig.config.active_cap.soft
        active_cap_hard = $remoteScoutConfig.config.active_cap.hard
        verifier_queue_cap = $remoteHealth.verifier_queue_cap
    }
}

$mismatches = @()
foreach ($key in @("rust_version", "model_id", "race_timeout_ms", "scout_profile", "release_profile", "backpressure_start_queue_depth", "admission_queue_depth_soft", "admission_queue_depth_hard", "active_cap_base", "active_cap_soft", "active_cap_hard", "verifier_queue_cap")) {
    if ($summary.local[$key] -ne $summary.remote[$key]) {
        $mismatches += $key
    }
}

[pscustomobject]@{
    ok = ($mismatches.Count -eq 0)
    mismatches = $mismatches
    summary = $summary
} | ConvertTo-Json -Depth 6
