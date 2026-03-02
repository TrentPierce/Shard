$ErrorActionPreference = "Stop"

param(
    [string]$InstallDir = "$env:ProgramFiles\ShardNode",
    [int]$ControlPort = 9091,
    [string]$BootstrapHealthUrl = "http://127.0.0.1:9091/v1/system/bootstrap",
    [switch]$DownloadModelIfMissing,
    [switch]$NonInteractive,
    [switch]$Gui
)

function Ensure-Dirs {
    param([string]$DataDir, [string]$IdentityDir, [string]$ModelDir)
    New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
    New-Item -ItemType Directory -Path $IdentityDir -Force | Out-Null
    New-Item -ItemType Directory -Path $ModelDir -Force | Out-Null
}

function Download-ModelIfRequested {
    param([bool]$ShouldDownload, [string]$ModelPath)
    if (Test-Path $ModelPath) {
        return "Model present"
    }

    if (-not $ShouldDownload) {
        return "Model missing (download skipped)"
    }

    $modelUrl = "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf"
    try {
        Invoke-WebRequest -Uri $modelUrl -OutFile $ModelPath
        return "Model downloaded"
    } catch {
        return "Model download failed: $($_.Exception.Message)"
    }
}

function Ensure-FirewallRule {
    param([int]$Port, [bool]$ShouldCreate)
    $ruleName = "Shard Node Control Plane"
    if (Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue) {
        return "Firewall rule already exists"
    }

    if (-not $ShouldCreate) {
        return "Firewall rule creation skipped"
    }

    try {
        New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Protocol TCP -LocalPort $Port -Action Allow | Out-Null
        return "Firewall rule created"
    } catch {
        return "Firewall rule failed: $($_.Exception.Message)"
    }
}

function Check-Health {
    param([int]$Port)
    try {
        $response = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/health" -TimeoutSec 10
        if ($response.status -eq "ok") {
            return "Health check passed"
        }
        return "Health check returned unexpected payload"
    } catch {
        return "Health check failed: $($_.Exception.Message)"
    }
}

function Check-Bootstrap {
    param([string]$Url)
    try {
        $bootstrap = Invoke-RestMethod -Uri $Url -TimeoutSec 10
        if ($bootstrap.local_peer_id -or $bootstrap.ok -or $bootstrap.known_bootstraps) {
            return "Bootstrap endpoint reachable"
        }
        return "Bootstrap endpoint reachable (unexpected payload)"
    } catch {
        return "Bootstrap connectivity failed: $($_.Exception.Message)"
    }
}

function Start-GuiWizard {
    param(
        [string]$ModelPath,
        [int]$Port,
        [string]$BootstrapUrl
    )

    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing

    $form = New-Object System.Windows.Forms.Form
    $form.Text = "Shard First-Run Wizard"
    $form.Size = New-Object System.Drawing.Size(560, 420)
    $form.StartPosition = "CenterScreen"
    $form.FormBorderStyle = "FixedDialog"
    $form.MaximizeBox = $false
    $form.MinimizeBox = $false

    $title = New-Object System.Windows.Forms.Label
    $title.Text = "Welcome to Shard Node setup"
    $title.Font = New-Object System.Drawing.Font("Segoe UI", 12, [System.Drawing.FontStyle]::Bold)
    $title.AutoSize = $true
    $title.Location = New-Object System.Drawing.Point(20, 20)
    $form.Controls.Add($title)

    $modelBox = New-Object System.Windows.Forms.CheckBox
    $modelBox.Text = "Download baseline model if missing"
    $modelBox.Checked = -not (Test-Path $ModelPath)
    $modelBox.AutoSize = $true
    $modelBox.Location = New-Object System.Drawing.Point(24, 70)
    $form.Controls.Add($modelBox)

    $firewallBox = New-Object System.Windows.Forms.CheckBox
    $firewallBox.Text = "Create firewall allow rule for port $Port"
    $firewallBox.Checked = $true
    $firewallBox.AutoSize = $true
    $firewallBox.Location = New-Object System.Drawing.Point(24, 100)
    $form.Controls.Add($firewallBox)

    $status = New-Object System.Windows.Forms.TextBox
    $status.Multiline = $true
    $status.ReadOnly = $true
    $status.ScrollBars = "Vertical"
    $status.Size = New-Object System.Drawing.Size(500, 200)
    $status.Location = New-Object System.Drawing.Point(24, 140)
    $form.Controls.Add($status)

    $runButton = New-Object System.Windows.Forms.Button
    $runButton.Text = "Run checks"
    $runButton.Size = New-Object System.Drawing.Size(120, 30)
    $runButton.Location = New-Object System.Drawing.Point(264, 350)
    $form.Controls.Add($runButton)

    $closeButton = New-Object System.Windows.Forms.Button
    $closeButton.Text = "Finish"
    $closeButton.Size = New-Object System.Drawing.Size(120, 30)
    $closeButton.Location = New-Object System.Drawing.Point(404, 350)
    $closeButton.Add_Click({ $form.Close() })
    $form.Controls.Add($closeButton)

    $runButton.Add_Click({
        $status.Clear()
        $status.AppendText("Preparing node directories...`r`n")

        $downloadResult = Download-ModelIfRequested -ShouldDownload $modelBox.Checked -ModelPath $ModelPath
        $status.AppendText("$downloadResult`r`n")

        $firewallResult = Ensure-FirewallRule -Port $Port -ShouldCreate $firewallBox.Checked
        $status.AppendText("$firewallResult`r`n")

        Start-Sleep -Seconds 2

        $healthResult = Check-Health -Port $Port
        $status.AppendText("$healthResult`r`n")

        $bootstrapResult = Check-Bootstrap -Url $BootstrapUrl
        $status.AppendText("$bootstrapResult`r`n")
    })

    [void]$form.ShowDialog()
}

Write-Host "Shard First-Run Onboarding" -ForegroundColor Cyan

$dataDir = Join-Path $env:ProgramData "Shard"
$identityDir = Join-Path $dataDir "identity"
$modelDir = Join-Path $dataDir "models"
Ensure-Dirs -DataDir $dataDir -IdentityDir $identityDir -ModelDir $modelDir

if (-not $env:BITNET_MODEL) {
    $env:BITNET_MODEL = Join-Path $modelDir "model.gguf"
}

if ($Gui -and -not $NonInteractive) {
    Start-GuiWizard -ModelPath $env:BITNET_MODEL -Port $ControlPort -BootstrapUrl $BootstrapHealthUrl
    exit 0
}

if (-not (Test-Path $env:BITNET_MODEL)) {
    Write-Host "Model not found at $($env:BITNET_MODEL)." -ForegroundColor Yellow
    if (-not $NonInteractive -and -not $DownloadModelIfMissing) {
        $answer = Read-Host "Download baseline model now? (y/N)"
        if ($answer -match "^(y|yes)$") {
            $DownloadModelIfMissing = $true
        }
    }
    if ($DownloadModelIfMissing) {
        $downloadResult = Download-ModelIfRequested -ShouldDownload $true -ModelPath $env:BITNET_MODEL
        Write-Host $downloadResult
    } else {
        Write-Host "Download a model before contribution or set BITNET_MODEL to an existing path." -ForegroundColor Yellow
    }
}

Write-Host "Checking firewall rule..."
$createRule = $true
if (-not $NonInteractive) {
    $firewallAnswer = Read-Host "Create firewall allow rule for port $ControlPort? (Y/n)"
    if ($firewallAnswer -match "^(n|no)$") {
        $createRule = $false
    }
}
Write-Host (Ensure-FirewallRule -Port $ControlPort -ShouldCreate $createRule)

Write-Host "Starting service health check..."
Start-Sleep -Seconds 2
Write-Host (Check-Health -Port $ControlPort)

Write-Host "Checking bootstrap connectivity..."
Write-Host (Check-Bootstrap -Url $BootstrapHealthUrl)
