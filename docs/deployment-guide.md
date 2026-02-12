# Deployment Guide

This guide covers deploying Shard Oracle nodes in various environments, from local development to production cloud deployments.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Platform-Specific Instructions](#platform-specific-instructions)
  - [Linux](#linux)
  - [Windows](#windows)
  - [macOS](#macos)
- [Systemd Service Setup](#systemd-service-setup)
- [Container Deployment](#container-deployment)
- [Cloud Deployment](#cloud-deployment)
  - [AWS](#aws)
  - [Google Cloud Platform (GCP)](#google-cloud-platform-gcp)
  - [Microsoft Azure](#microsoft-azure)
- [Monitoring and Observability](#monitoring-and-observability)
- [Zero-Downtime Updates](#zero-downtime-updates)
- [Security Hardening](#security-hardening)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

### Local Production-Like Launch

1. **Start the Rust daemon:**

```bash
cd desktop/rust
cargo build --release
./target/release/shard-daemon --control-port 9091 --tcp-port 4001 --bootstrap /ip4/<seed>/tcp/4001
```

2. **Start the Python API:**

```bash
cd desktop/python
pip install -r requirements.txt
SHARD_API_KEYS=prod-key SHARD_RATE_LIMIT_PER_MINUTE=120 python run.py --rust-url http://127.0.0.1:9091
```

---

## Platform-Specific Instructions

### Linux

#### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Python 3.11+
sudo apt update
sudo apt install python3.11 python3.11-venv python3-pip

# Install Node.js 18+
curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
sudo apt install -y nodejs

# Install build dependencies
sudo apt install build-essential pkg-config libssl-dev
```

#### Build

```bash
cd desktop/rust
cargo build --release
```

The binary will be at `target/release/shard-daemon`.

---

### Windows

#### Prerequisites

1. **Install Rust:**
   - Download from [rustup.rs](https://rustup.rs)
   - Choose "Default installation" during setup
   - Restart your terminal after installation

2. **Install Python 3.11+:**
   - Download from [python.org](https://www.python.org/downloads/)
   - During installation, check **"Add Python to PATH"**
   - Verify installation:
     ```cmd
     python --version
     ```

3. **Install Visual Studio Build Tools:**
   - Download from [visualstudio.microsoft.com](https://visualstudio.microsoft.com/downloads/)
   - Install "Desktop development with C++" workload
   - Required for building Rust dependencies

4. **Install Node.js 18+:**
   - Download from [nodejs.org](https://nodejs.org/)
   - Choose the LTS version

#### Build

```cmd
cd desktop\rust
cargo build --release
```

The binary will be at `target\release\shard-daemon.exe`.

#### Run as Windows Service

Using **NSSM** (Non-Sucking Service Manager):

```cmd
# Download NSSM from https://nssm.cc/download
# Install as service for Rust daemon
nssm install ShardDaemon "C:\path\to\Shard\desktop\rust\target\release\shard-daemon.exe"
nssm set ShardDaemon AppParameters "--control-port 9091 --tcp-port 4001"
nssm set ShardDaemon AppDirectory "C:\path\to\Shard\desktop\rust"
nssm start ShardDaemon

# Install as service for Python API
nssm install ShardAPI "C:\Python311\python.exe"
nssm set ShardAPI AppParameters "C:\path\to\Shard\desktop\python\run.py --port 8000"
nssm set ShardAPI AppDirectory "C:\path\to\Shard\desktop\python"
nssm set ShardAPI AppEnvironmentExtra "SHARD_API_KEYS=prod-key"
nssm start ShardAPI
```

#### PowerShell Scripts

Create `start-shard.ps1`:

```powershell
# Start Rust Daemon
$rustProcess = Start-Process -FilePath "target\release\shard-daemon.exe" -ArgumentList "--control-port 9091 --tcp-port 4001" -PassThru

# Wait for daemon to start
Start-Sleep -Seconds 2

# Start Python API
$env:SHARD_API_KEYS = "prod-key"
$env:SHARD_RATE_LIMIT_PER_MINUTE = "120"
$apiProcess = Start-Process -FilePath "python" -ArgumentList "run.py --port 8000" -PassThru

Write-Host "Shard services started (Rust: $($rustProcess.Id), API: $($apiProcess.Id))"
```

#### Firewall Configuration

Allow required ports:

```cmd
# Allow Python API port
netsh advfirewall firewall add rule name="Shard API" dir=in action=allow protocol=TCP localport=8000

# Allow Rust daemon control plane
netsh advfirewall firewall add rule name="Shard Daemon Control" dir=in action=allow protocol=TCP localport=9091

# Allow P2P TCP
netsh advfirewall firewall add rule name="Shard P2P TCP" dir=in action=allow protocol=TCP localport=4001

# Allow P2P WebSocket
netsh advfirewall firewall add rule name="Shard P2P WebSocket" dir=in action=allow protocol=TCP localport=4101
```

---

### macOS

#### Prerequisites

```bash
# Install Homebrew (if not already installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install Rust
brew install rust

# Install Python 3.11+
brew install python@3.11

# Install Node.js 18+
brew install node

# Install OpenSSL (required for some Rust dependencies)
brew install openssl pkg-config
```

#### Build

```bash
cd desktop/rust
cargo build --release
```

The binary will be at `target/release/shard-daemon`.

#### Launch Agent Setup

Create a macOS Launch Agent for the Rust daemon at `~/Library/LaunchAgents/com.shard.daemon.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.shard.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/youruser/Shard/desktop/rust/target/release/shard-daemon</string>
        <string>--control-port</string>
        <string>9091</string>
        <string>--tcp-port</string>
        <string>4001</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>WorkingDirectory</key>
    <string>/Users/youruser/Shard/desktop/rust</string>
    <key>StandardOutPath</key>
    <string>/Users/youruser/Library/Logs/com.shard.daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/Users/youruser/Library/Logs/com.shard.daemon.err</string>
</dict>
</plist>
```

Load the agent:

```bash
launchctl load ~/Library/LaunchAgents/com.shard.daemon.plist
```

---

## Systemd Service Setup

For Linux deployments, use systemd for automatic startup and restart.

### Rust Daemon Service

Create `/etc/systemd/system/shard-daemon.service`:

```ini
[Unit]
Description=Shard Oracle P2P Daemon
After=network.target

[Service]
Type=simple
User=shard
Group=shard
WorkingDirectory=/opt/shard/rust
ExecStart=/opt/shard/rust/target/release/shard-daemon \
  --control-port 9091 \
  --tcp-port 4001 \
  --bootstrap /ip4/192.168.1.10/tcp/4001 \
  --reconnect-seconds 20 \
  --log-level info
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/shard/data

[Install]
WantedBy=multi-user.target
```

### Python API Service

Create `/etc/systemd/system/shard-api.service`:

```ini
[Unit]
Description=Shard Oracle API
Requires=shard-daemon.service
After=shard-daemon.service

[Service]
Type=simple
User=shard
Group=shard
WorkingDirectory=/opt/shard/python
Environment="PATH=/opt/shard/python/venv/bin"
Environment="SHARD_API_KEYS=prod-key-1,prod-key-2"
Environment="SHARD_RATE_LIMIT_PER_MINUTE=120"
Environment="SHARD_MAX_PROMPT_CHARS=16000"
Environment="SHARD_LOG_LEVEL=INFO"
Environment="SHARD_RUST_URL=http://127.0.0.1:9091"
ExecStart=/opt/shard/python/venv/bin/python run.py --port 8000
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

### Enable and Start Services

```bash
# Create user
sudo useradd -r -s /bin/false shard
sudo mkdir -p /opt/shard/{rust,python,data}
sudo chown -R shard:shard /opt/shard

# Reload systemd
sudo systemctl daemon-reload

# Enable services (start on boot)
sudo systemctl enable shard-daemon shard-api

# Start services
sudo systemctl start shard-daemon shard-api

# Check status
sudo systemctl status shard-daemon shard-api

# View logs
sudo journalctl -u shard-daemon -f
sudo journalctl -u shard-api -f
```

---

## Container Deployment

### Dockerfile

The project includes a Dockerfile for containerized deployments.

#### Build Image

```bash
docker build -t shard:latest .
docker build -t shard:0.4.0 .
```

#### Run Container

```bash
docker run --rm \
  --name shard-oracle \
  -p 8000:8000 \
  -p 9091:9091 \
  -p 4001:4001 \
  -p 4101:4101 \
  -e SHARD_API_KEYS=prod-key \
  -e SHARD_RATE_LIMIT_PER_MINUTE=120 \
  -e SHARD_LOG_LEVEL=INFO \
  -v shard-data:/opt/shard/data \
  shard:latest
```

#### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  shard-daemon:
    build:
      context: .
      dockerfile: Dockerfile.daemon
    ports:
      - "9091:9091"
      - "4001:4001"
      - "4101:4101"
    volumes:
      - shard-data:/opt/shard/data
    environment:
      - RUST_LOG=info
    restart: unless-stopped

  shard-api:
    build:
      context: .
      dockerfile: Dockerfile.api
    ports:
      - "8000:8000"
    depends_on:
      - shard-daemon
    environment:
      - SHARD_RUST_URL=http://shard-daemon:9091
      - SHARD_API_KEYS=prod-key
      - SHARD_RATE_LIMIT_PER_MINUTE=120
      - SHARD_LOG_LEVEL=INFO
    restart: unless-stopped

volumes:
  shard-data:
```

Run with Docker Compose:

```bash
docker-compose up -d
docker-compose logs -f
```

---

## Cloud Deployment

### AWS

#### EC2 Deployment

1. **Launch EC2 Instance:**
   - Instance type: `g4dn.xlarge` (NVIDIA T4) or `g5.xlarge` (A10G)
   - AMI: Ubuntu 22.04 LTS
   - Storage: 50GB GP3

2. **Security Group Rules:**
   ```
   Inbound:
   - TCP 22   (SSH)    -> Your IP
   - TCP 8000 (API)    -> 0.0.0.0/0 (or restrict to known IPs)
   - TCP 9091 (Control)-> 0.0.0.0/0 (or restrict)
   - TCP 4001 (P2P)    -> 0.0.0.0/0
   - TCP 4101 (WS)     -> 0.0.0.0/0
   ```

3. **SSH into Instance:**

```bash
ssh -i your-key.pem ubuntu@your-ec2-ip
```

4. **Install Dependencies:**

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev python3.11 python3.11-venv python3-pip curl git

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install NVIDIA drivers (if using GPU instances)
sudo apt install -y nvidia-driver-535 nvidia-utils-535

# Clone and build
git clone https://github.com/ShardNetwork/Shard.git
cd Shard
cd desktop/rust
cargo build --release
```

5. **Setup systemd services** (see [Systemd Service Setup](#systemd-service-setup))

#### Application Load Balancer

For high availability, use ALB with multiple instances:

```bash
# Create target group
aws elbv2 create-target-group \
  --name shard-api-tg \
  --protocol HTTP \
  --port 8000 \
  --vpc-id your-vpc-id

# Register instances
aws elbv2 register-targets \
  --target-group-arn your-tg-arn \
  --targets Id=i-instance1 Id=i-instance2

# Create load balancer
aws elbv2 create-load-balancer \
  --name shard-alb \
  --subnets subnet-1 subnet-2 \
  --security-groups sg-12345

# Create listener
aws elbv2 create-listener \
  --load-balancer-arn your-alb-arn \
  --protocol HTTP \
  --port 443 \
  --default-actions Type=forward,TargetGroupArn=your-tg-arn
```

#### Auto Scaling Group

```bash
# Create launch template
aws ec2 create-launch-template \
  --name shard-launch-template \
  --image-id ami-12345 \
  --instance-type g4dn.xlarge \
  --key-name your-key \
  --user-data file://user-data.sh

# Create auto scaling group
aws autoscaling create-auto-scaling-group \
  --auto-scaling-group-name shard-asg \
  --launch-template TemplateId=lt-12345 \
  --min-size 2 \
  --max-size 10 \
  --desired-capacity 2 \
  --vpc-zone-identifier subnet-1,subnet-2
```

---

### Google Cloud Platform (GCP)

#### GCE Deployment

1. **Create Compute Engine Instance:**

```bash
gcloud compute instances create shard-oracle \
  --zone=us-central1-a \
  --machine-type=n1-standard-4 \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud-cloud \
  --boot-disk-size=50GB \
  --boot-disk-type=pd-ssd \
  --tags=shard-api,shard-p2p
```

2. **Configure Firewall:**

```bash
gcloud compute firewall-rules create shard-api \
  --allow tcp:8000 \
  --source-ranges=0.0.0.0/0 \
  --description="Allow Shard API"

gcloud compute firewall-rules create shard-p2p \
  --allow tcp:4001,tcp:4101,tcp:9091 \
  --source-ranges=0.0.0.0/0 \
  --description="Allow Shard P2P networking"
```

3. **SSH and Deploy:**

```bash
gcloud compute ssh shard-oracle --zone=us-central1-a
# Follow Linux deployment steps
```

#### Cloud Run (Container)

Build and deploy to Cloud Run:

```bash
# Build and push image
gcloud builds submit --tag gcr.io/your-project/shard:latest

# Deploy
gcloud run deploy shard-oracle \
  --image gcr.io/your-project/shard:latest \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --port 8000 \
  --set-env-vars SHARD_API_KEYS=prod-key,SHARD_RATE_LIMIT_PER_MINUTE=120
```

---

### Microsoft Azure

#### Azure VM Deployment

1. **Create VM:**

```bash
az vm create \
  --resource-group shard-rg \
  --name shard-oracle \
  --image Ubuntu2204 \
  --size Standard_NC4as_T4_v3 \
  --admin-username azureuser \
  --ssh-key-values ~/.ssh/id_rsa.pub \
  --location eastus
```

2. **Open Ports:**

```bash
az network nsg rule create \
  --resource-group shard-rg \
  --nsg-name shard-oracleNSG \
  --name AllowAPI \
  --access Allow \
  --protocol Tcp \
  --direction Inbound \
  --priority 1000 \
  --source-address-prefixes '*' \
  --source-port-ranges '*' \
  --destination-address-prefixes '*' \
  --destination-port-ranges 8000

# Repeat for ports 9091, 4001, 4101
```

3. **SSH and Deploy:**

```bash
ssh azureuser@shard-oracle.eastus.cloudapp.azure.com
# Follow Linux deployment steps
```

#### Azure Container Instances

```bash
# Create resource group
az group create --name shard-rg --location eastus

# Create container instance
az container create \
  --resource-group shard-rg \
  --name shard-oracle \
  --image shard:latest \
  --cpu 4 \
  --memory 8 \
  --ports 8000 9091 4001 4101 \
  --environment-variables SHARD_API_KEYS=prod-key SHARD_RATE_LIMIT_PER_MINUTE=120
```

---

## Monitoring and Observability

### Health Checks

```bash
# API health
curl http://localhost:8000/health

# Rust daemon health
curl http://localhost:9091/health

# Network topology
curl http://localhost:8000/v1/system/topology

# Connected peers
curl http://localhost:8000/v1/system/peers
```

### Prometheus Metrics

Access metrics at `/metrics` endpoint:

```bash
curl http://localhost:8000/metrics
```

Prometheus configuration example:

```yaml
scrape_configs:
  - job_name: 'shard'
    static_configs:
      - targets: ['localhost:8000']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### Logging

#### Systemd Journal

```bash
# View Rust daemon logs
sudo journalctl -u shard-daemon -f

# View Python API logs
sudo journalctl -u shard-api -f

# View last 100 lines
sudo journalctl -u shard-api -n 100

# View logs since boot
sudo journalctl -u shard-api --boot
```

#### Docker Logs

```bash
# View logs
docker logs shard-oracle -f

# View last 100 lines
docker logs shard-oracle --tail 100
```

---

## Zero-Downtime Updates

### Rolling Deployment Strategy

For production deployments with multiple instances:

1. **Deploy to standby instances first**
2. **Run health checks** on new version
3. **Drain traffic** from old instances
4. **Terminate old instances** only after confirming new ones are healthy

### Health Check Script

Create `health_check.sh`:

```bash
#!/bin/bash

# Check API health
if curl -f http://localhost:8000/health > /dev/null 2>&1; then
    echo "API: OK"
else
    echo "API: FAILED"
    exit 1
fi

# Check Rust daemon
if curl -f http://localhost:9091/health > /dev/null 2>&1; then
    echo "Rust Daemon: OK"
else
    echo "Rust Daemon: FAILED"
    exit 1
fi

# Check connected peers
PEER_COUNT=$(curl -s http://localhost:9091/health | jq -r '.connected_peers')
if [ "$PEER_COUNT" -gt 0 ]; then
    echo "Peers: $PEER_COUNT connected"
else
    echo "Warning: No peers connected"
fi

echo "All health checks passed"
exit 0
```

### Update Procedure

```bash
# 1. Pull latest code
git pull origin main

# 2. Build new version
cd desktop/rust
cargo build --release

# 3. Run health check on new binary
./target/release/shard-daemon --help

# 4. Stop old service
sudo systemctl stop shard-daemon

# 5. Backup old binary
sudo cp target/release/shard-daemon target/release/shard-daemon.old

# 6. Start new service
sudo systemctl start shard-daemon

# 7. Verify health
./health_check.sh

# 8. If healthy, restart API
sudo systemctl restart shard-api

# 9. Final verification
./health_check.sh
```

---

## Security Hardening

### Environment Variables

Set these for production:

```bash
# API Keys (comma-separated)
export SHARD_API_KEYS="secure-key-1,secure-key-2"

# Rate limiting
export SHARD_RATE_LIMIT_PER_MINUTE=120

# Max prompt size
export SHARD_MAX_PROMPT_CHARS=16000

# Log level
export SHARD_LOG_LEVEL=INFO

# CORS origins (restrict to known domains)
export SHARD_CORS_ORIGINS="https://yourdomain.com,https://app.yourdomain.com"
```

### Firewall Configuration

Restrict access to only necessary ports and IPs:

```bash
# Allow only from trusted network for control plane
sudo ufw allow from 10.0.0.0/8 to any port 9091

# Allow API from CDN or load balancer only
sudo ufw allow from 203.0.113.0/24 to any port 8000

# Allow P2P from anywhere (required for network)
sudo ufw allow 4001/tcp
sudo ufw allow 4101/tcp
```

### TLS/SSL

Use a reverse proxy with TLS:

#### Nginx Configuration

```nginx
server {
    listen 443 ssl http2;
    server_name api.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/api.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.yourdomain.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://localhost:8000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name api.yourdomain.com;
    return 301 https://$server_name$request_uri;
}
```

---

## Troubleshooting

For common issues, see [`docs/troubleshooting.md`](troubleshooting.md).

### Common Deployment Issues

#### Rust daemon fails to start

Check logs:
```bash
sudo journalctl -u shard-daemon -n 50
```

Common causes:
- Port already in use
- Network configuration issues
- Insufficient permissions

#### API cannot connect to Rust daemon

Verify Rust daemon is running:
```bash
curl http://localhost:9091/health
```

Check SHARD_RUST_URL environment variable.

#### P2P connection failures

- Verify firewall allows ports 4001 and 4101
- Check bootstrap peer address is correct
- Review daemon logs for connection errors

---

For additional support, see the main [README](../README.md) or [CONTRIBUTING.md](../CONTRIBUTING.md).
