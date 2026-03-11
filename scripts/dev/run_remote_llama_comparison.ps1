param(
    [string]$BaseUrl = "http://127.0.0.1:9191",
    [string]$Model = "meta-llama/Llama-3.1-8B",
    [int]$MaxTokens = 64,
    [int]$BaselineRuns = 10,
    [int]$DistributedRuns = 10,
    [string]$Prompt = "Write one short paragraph explaining why peer-to-peer AI networks matter.",
    [string]$OutputPath = "runtime\\remote_llama_comparison_10v10.json"
)

$ErrorActionPreference = "Stop"

function Invoke-CurlJson {
    param(
        [string[]]$ArgsList
    )

    $output = & curl.exe @ArgsList
    $exitCode = $LASTEXITCODE
    [pscustomobject]@{
        Output   = ($output -join "`n")
        ExitCode = $exitCode
    }
}

function New-RequestBodyFile {
    param(
        [string]$Path,
        [string]$ModelName,
        [int]$TokenCount,
        [string]$UserPrompt
    )

    $json = @{
        model = $ModelName
        stream = $false
        max_tokens = $TokenCount
        messages = @(
            @{
                role = "user"
                content = $UserPrompt
            }
        )
    } | ConvertTo-Json -Depth 6 -Compress

    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText((Join-Path (Get-Location) $Path), $json, $utf8NoBom)
}

function Invoke-OneRun {
    param(
        [string]$Mode,
        [int]$Index,
        [string]$TargetBaseUrl,
        [string]$BodyPath
    )

    Invoke-RestMethod -Uri "$TargetBaseUrl/v1/system/speculative-trace/reset" -Method Post | Out-Null
    Start-Sleep -Milliseconds 200

    $args = @(
        "-s",
        "-X", "POST",
        "$TargetBaseUrl/v1/chat/completions",
        "-H", "Content-Type: application/json",
        "-H", "x-shard-mesh-forward: false"
    )

    if ($Mode -eq "distributed") {
        $args += @("-H", "x-shard-inference-mode: distributed")
    }

    $args += @("--data-binary", "@$BodyPath")

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $curl = Invoke-CurlJson -ArgsList $args
    $sw.Stop()

    Start-Sleep -Seconds 5

    $trace = Invoke-RestMethod -Uri "$TargetBaseUrl/v1/system/speculative-trace"
    $summary = Invoke-RestMethod -Uri "$TargetBaseUrl/metrics/summary"

    $response = $null
    try {
        $response = $curl.Output | ConvertFrom-Json
    } catch {
        $response = [pscustomobject]@{
            raw = $curl.Output
        }
    }

    $events = @($trace.events)
    $waitHit = $events | Where-Object { $_.stage -eq "wait_hit_mailbox" } | Select-Object -Last 1
    $waitTimeout = $events | Where-Object { $_.stage -eq "wait_timeout" } | Select-Object -Last 1
    $verify = $events | Where-Object { $_.stage -eq "verify_completed" } | Select-Object -Last 1
    $responseDone = $events | Where-Object { $_.stage -eq "response_completed" } | Select-Object -Last 1

    [pscustomobject]@{
        mode = $Mode
        run = $Index
        latency_ms = [math]::Round($sw.Elapsed.TotalMilliseconds, 3)
        curl_exit = $curl.ExitCode
        response = $response
        trace = $trace
        summary = $summary
        quick = [pscustomobject]@{
            response_model = $response.model
            completion_tokens = $response.usage.completion_tokens
            wait_hit = ($null -ne $waitHit)
            wait_timeout = ($null -ne $waitTimeout)
            wait_hit_ms = if ($waitHit) { $waitHit.pending_age_ms } else { $null }
            verify_detail = if ($verify) { $verify.detail } else { $null }
            response_detail = if ($responseDone) { $responseDone.detail } else { $null }
        }
    }
}

$bodyPath = "runtime\\tmp_remote_llama_req.json"
New-RequestBodyFile -Path $bodyPath -ModelName $Model -TokenCount $MaxTokens -UserPrompt $Prompt

$results = [System.Collections.Generic.List[object]]::new()

foreach ($i in 1..$BaselineRuns) {
    Write-Host "Running baseline $i/$BaselineRuns..."
    $results.Add((Invoke-OneRun -Mode "baseline" -Index $i -TargetBaseUrl $BaseUrl -BodyPath $bodyPath)) | Out-Null
}

foreach ($i in 1..$DistributedRuns) {
    Write-Host "Running distributed $i/$DistributedRuns..."
    $results.Add((Invoke-OneRun -Mode "distributed" -Index $i -TargetBaseUrl $BaseUrl -BodyPath $bodyPath)) | Out-Null
}

$dir = Split-Path -Parent $OutputPath
if ($dir) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

$results | ConvertTo-Json -Depth 20 | Set-Content -Path $OutputPath -Encoding UTF8

$summaryView = $results | Select-Object mode, run, latency_ms, @{n="response_model";e={$_.quick.response_model}}, @{n="completion_tokens";e={$_.quick.completion_tokens}}, @{n="wait_hit";e={$_.quick.wait_hit}}, @{n="wait_timeout";e={$_.quick.wait_timeout}}, @{n="wait_hit_ms";e={$_.quick.wait_hit_ms}}, @{n="verify_detail";e={$_.quick.verify_detail}}, @{n="response_detail";e={$_.quick.response_detail}}

Write-Host ""
Write-Host "Saved results to $OutputPath"
$summaryView | Format-Table -AutoSize
