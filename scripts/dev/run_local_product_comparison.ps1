param(
    [string]$BaseUrl = "http://127.0.0.1:9191",
    [string]$Model = "meta-llama/Llama-3.1-8B",
    [int]$MaxTokens = 128,
    [int]$StandardRuns = 10,
    [int]$LocalSpeculativeRuns = 10,
    [string]$Prompt = "Explain in two short paragraphs how Shard routes easy prompts locally and escalates harder prompts to verifier nodes.",
    [double]$Temperature = 0.0,
    [double]$TopP = 1.0,
    [Nullable[int]]$Seed = 42,
    [string]$OutputPath = "runtime\\local_product_comparison_10v10_seed42.json"
)

$ErrorActionPreference = "Stop"

function Invoke-CurlJson {
    param(
        [string[]]$ArgsList
    )

    $previousEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & curl.exe @ArgsList 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousEap
    }
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
        [string]$UserPrompt,
        [double]$RequestTemperature,
        [double]$RequestTopP,
        [Nullable[int]]$RequestSeed
    )

    $json = [ordered]@{
        model = $ModelName
        stream = $false
        max_tokens = $TokenCount
        temperature = $RequestTemperature
        top_p = $RequestTopP
        messages = @(
            @{
                role = "user"
                content = $UserPrompt
            }
        )
    }

    if ($null -ne $RequestSeed) {
        $json.seed = [int]$RequestSeed
    }

    $json = $json | ConvertTo-Json -Depth 6 -Compress

    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText((Join-Path (Get-Location) $Path), $json, $utf8NoBom)
}

function Reset-SpeculativeTrace {
    param([string]$TargetBaseUrl)
    try {
        Invoke-RestMethod -Uri "$TargetBaseUrl/v1/system/speculative-trace/reset" -Method Post | Out-Null
    } catch {
        Write-Verbose "Trace reset unavailable: $($_.Exception.Message)"
    }
}

function Get-LatencyStats {
    param(
        [double[]]$Values
    )

    if (-not $Values -or $Values.Count -eq 0) {
        return [pscustomobject]@{
            avg_ms = 0
            p50_ms = 0
            p95_ms = 0
            min_ms = 0
            max_ms = 0
        }
    }

    $sorted = $Values | Sort-Object
    $avg = ($Values | Measure-Object -Average).Average
    $p50Index = [Math]::Min($sorted.Count - 1, [Math]::Round(($sorted.Count - 1) * 0.50))
    $p95Index = [Math]::Min($sorted.Count - 1, [Math]::Round(($sorted.Count - 1) * 0.95))
    [pscustomobject]@{
        avg_ms = [math]::Round([double]$avg, 3)
        p50_ms = [math]::Round([double]$sorted[$p50Index], 3)
        p95_ms = [math]::Round([double]$sorted[$p95Index], 3)
        min_ms = [math]::Round([double]$sorted[0], 3)
        max_ms = [math]::Round([double]$sorted[-1], 3)
    }
}

function Invoke-OneRun {
    param(
        [string]$Mode,
        [int]$Index,
        [string]$TargetBaseUrl,
        [string]$BodyPath
    )

    Reset-SpeculativeTrace -TargetBaseUrl $TargetBaseUrl
    Start-Sleep -Milliseconds 200

    $args = @(
        "-s",
        "-S",
        "-X", "POST",
        "$TargetBaseUrl/v1/chat/completions",
        "-H", "Content-Type: application/json",
        "-H", "x-shard-mesh-forward: false",
        "-H", "x-shard-inference-mode: $Mode",
        "-w", "`n__CURL_HTTP_CODE__:%{http_code}",
        "--data-binary", "@$BodyPath"
    )

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $curl = Invoke-CurlJson -ArgsList $args
    $sw.Stop()

    Start-Sleep -Milliseconds 750

    $trace = $null
    try {
        $trace = Invoke-RestMethod -Uri "$TargetBaseUrl/v1/system/speculative-trace"
    } catch {
        $trace = [pscustomobject]@{ events = @() }
    }

    $rawOutput = $curl.Output
    $httpCode = $null
    $httpCodeMatch = [regex]::Match($rawOutput, "__CURL_HTTP_CODE__:(\d{3})\s*$")
    if ($httpCodeMatch.Success) {
        $httpCode = [int]$httpCodeMatch.Groups[1].Value
        $rawOutput = [regex]::Replace($rawOutput, "`r?`n__CURL_HTTP_CODE__:\d{3}\s*$", "")
    }

    $response = $null
    try {
        $response = $rawOutput | ConvertFrom-Json
    } catch {
        $response = [pscustomobject]@{
            raw = $rawOutput
        }
    }

    $events = @($trace.events)
    $verify = $events | Where-Object { $_.stage -eq "verify_completed" } | Select-Object -Last 1
    $responseDone = $events | Where-Object { $_.stage -eq "response_completed" } | Select-Object -Last 1
    $speculativeBypass = $events | Where-Object { $_.stage -eq "speculative_bypassed" } | Select-Object -Last 1

    $responseMetrics = @{
        request_total_ms = $null
        generation_ms = $null
        accepted_tokens = $null
        completion_tokens_generated = $null
    }
    if ($responseDone -and $responseDone.detail) {
        foreach ($pair in ($responseDone.detail -split ",")) {
            $kv = $pair -split "=", 2
            if ($kv.Count -ne 2) {
                continue
            }
            $key = $kv[0].Trim()
            $value = $kv[1].Trim()
            switch ($key) {
                "request_total_ms" { $responseMetrics.request_total_ms = [double]$value }
                "generation_ms" { $responseMetrics.generation_ms = [double]$value }
                "accepted_tokens" { $responseMetrics.accepted_tokens = [int]$value }
                "completion_tokens_generated" { $responseMetrics.completion_tokens_generated = [int]$value }
            }
        }
    }

    $completionTokens = $responseMetrics.completion_tokens_generated
    if (($response.PSObject.Properties.Name -contains "usage") -and $null -ne $response.usage) {
        if ($response.usage.PSObject.Properties.Name -contains "completion_tokens") {
            $completionTokens = $response.usage.completion_tokens
        }
    }

    [pscustomobject]@{
        mode = $Mode
        run = $Index
        latency_ms = if ($null -ne $responseMetrics.request_total_ms) {
            [math]::Round([double]$responseMetrics.request_total_ms, 3)
        } else {
            [math]::Round($sw.Elapsed.TotalMilliseconds, 3)
        }
        transport_latency_ms = [math]::Round($sw.Elapsed.TotalMilliseconds, 3)
        curl_http_code = $httpCode
        curl_exit = $curl.ExitCode
        completion_tokens = $completionTokens
        accepted_tokens = $responseMetrics.accepted_tokens
        generation_ms = $responseMetrics.generation_ms
        trace_success = ($null -ne $responseDone)
        verify_detail = if ($verify) { $verify.detail } else { $null }
        bypass_detail = if ($speculativeBypass) { $speculativeBypass.detail } else { $null }
        trace = $trace
        response = $response
    }
}

$bodyPath = "runtime\\tmp_local_product_req.json"
New-RequestBodyFile -Path $BodyPath -ModelName $Model -TokenCount $MaxTokens -UserPrompt $Prompt -RequestTemperature $Temperature -RequestTopP $TopP -RequestSeed $Seed

$results = [System.Collections.Generic.List[object]]::new()

foreach ($i in 1..$StandardRuns) {
    Write-Host "Running standard $i/$StandardRuns..."
    $results.Add((Invoke-OneRun -Mode "standard" -Index $i -TargetBaseUrl $BaseUrl -BodyPath $bodyPath)) | Out-Null
}

foreach ($i in 1..$LocalSpeculativeRuns) {
    Write-Host "Running local_speculative $i/$LocalSpeculativeRuns..."
    $results.Add((Invoke-OneRun -Mode "local_speculative" -Index $i -TargetBaseUrl $BaseUrl -BodyPath $bodyPath)) | Out-Null
}

$dir = Split-Path -Parent $OutputPath
if ($dir) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

$modeGroups = $results | Group-Object mode
$summary = foreach ($group in $modeGroups) {
    $latencies = @($group.Group | ForEach-Object { [double]$_.latency_ms })
    $stats = Get-LatencyStats -Values $latencies
    $acceptedTokenSum = ($group.Group | Measure-Object -Property accepted_tokens -Sum).Sum
    if ($null -eq $acceptedTokenSum) {
        $acceptedTokenSum = 0
    }
    [pscustomobject]@{
        mode = $group.Name
        runs = $group.Count
        avg_ms = $stats.avg_ms
        p50_ms = $stats.p50_ms
        p95_ms = $stats.p95_ms
        min_ms = $stats.min_ms
        max_ms = $stats.max_ms
        accepted_tokens_total = $acceptedTokenSum
    }
}

$report = [pscustomobject]@{
    created_at = (Get-Date).ToString("o")
    base_url = $BaseUrl
    model = $Model
    max_tokens = $MaxTokens
    prompt = $Prompt
    temperature = $Temperature
    top_p = $TopP
    seed = $Seed
    results = $results
    summary = $summary
}

$report | ConvertTo-Json -Depth 20 | Set-Content -Path $OutputPath -Encoding UTF8

Write-Host ""
Write-Host "Saved results to $OutputPath"
$summary | Format-Table -AutoSize
