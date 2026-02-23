#!/bin/bash
# Shard Network - EC2 Deployment Script
# Usage: ./deploy-ec2.sh <EC2_HOST> <SSH_KEY_PATH>
#
# Prerequisites:
#   - EC2 instance running with Ubuntu 22.04+
#   - SSH access configured
#   - Ports open: 8000 (API), 3000 (Dashboard), 4101 (P2P WS), 4001 (P2P TCP)
#   - GitHub repo: https://github.com/TrentPierce/Shard

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
EC2_HOST="${1:-}"
SSH_KEY="${2:-~/.ssh/shard-ec2.pem}"
APP_DIR="/opt/shard"
PYTHON_VENV="$APP_DIR/venv"
SERVICE_USER="shard"

# Usage function
usage() {
    echo "Usage: $0 <EC2_HOST> <SSH_KEY_PATH>"
    echo ""
    echo "Arguments:"
    echo "  EC2_HOST      - IP or hostname of your EC2 instance"
    echo "  SSH_KEY_PATH  - Path to SSH private key (default: ~/.ssh/shard-ec2.pem)"
    echo ""
    echo "Example:"
    echo "  $0 54.123.45.67 ~/.ssh/my-key.pem"
    exit 1
}

# Check arguments
if [ -z "$EC2_HOST" ]; then
    echo -e "${RED}Error: EC2_HOST is required${NC}"
    usage
fi

echo -e "${GREEN}=== Shard Network EC2 Deployment ===${NC}"
echo "Target: $EC2_HOST"
echo "Key: $SSH_KEY"

# ─────────────────────────────────────────────────────────────
# Step 1: Create remote directories
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[1/8] Creating remote directories...${NC}"
ssh -i "$SSH_KEY" ubuntu@$EC2_HOST "
    sudo mkdir -p $APP_DIR/{rust,python,web,data,logs}
    sudo chown -R $SERVICE_USER:$SERVICE_USER $APP_DIR
"

# ─────────────────────────────────────────────────────────────
# Step 2: Transfer Python files
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[2/8] Transferring Python API...${NC}"
scp -i "$SSH_KEY" -r \
    desktop/python/* \
    ubuntu@$EC2_HOST:$APP_DIR/python/

# ─────────────────────────────────────────────────────────────
# Step 3: Transfer Web UI files
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[3/8] Transferring Web UI...${NC}"
scp -i "$SSH_KEY" -r \
    web/package.json web/next.config.js web/tsconfig.json \
    web/src ubuntu@$EC2_HOST:$APP_DIR/web/

# ─────────────────────────────────────────────────────────────
# Step 4: Install dependencies on EC2
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[4/8] Installing dependencies on EC2...${NC}"
ssh -i "$SSH_KEY" ubuntu@$EC2_HOST "
    # Create Python venv
    python3 -m venv $PYTHON_VENV
    source $PYTHON_VENV/bin/activate
    
    # Install Python dependencies
    pip install --upgrade pip
    pip install -r $APP_DIR/python/requirements.txt
    
    # Install Node.js dependencies for dashboard
    cd $APP_DIR/web
    npm install
"

# ─────────────────────────────────────────────────────────────
# Step 5: Build Web UI
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[5/8] Building Web UI...${NC}"
ssh -i "$SSH_KEY" ubuntu@$EC2_HOST "
    cd $APP_DIR/web
    npm run build
"

# ─────────────────────────────────────────────────────────────
# Step 6: Install and configure Nginx
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[6/8] Configuring Nginx...${NC}"
scp -i "$SSH_KEY" docs/nginx-telemetry-dashboard.conf ubuntu@$EC2_HOST:/tmp/shard-nginx.conf
ssh -i "$SSH_KEY" ubuntu@$EC2_HOST "
    sudo cp /tmp/shard-nginx.conf /etc/nginx/sites-available/shard
    sudo ln -sf /etc/nginx/sites-available/shard /etc/nginx/sites-enabled/
    sudo nginx -t
    sudo systemctl reload nginx
"

# ─────────────────────────────────────────────────────────────
# Step 7: Create systemd service files
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[7/8] Creating systemd services...${NC}"
ssh -i "$SSH_KEY" ubuntu@$EC2_HOST "
    # Rust Daemon service
    sudo tee /etc/systemd/system/shard-daemon.service > /dev/null << 'EOF'
[Unit]
Description=Shard Oracle P2P Daemon
After=network.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_USER
WorkingDirectory=$APP_DIR/rust
    ExecStart=$APP_DIR/rust/target/release/shard-daemon --control-port 9091 --tcp-port 4001 --public-api --relay-mode trueRestart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    # Python API service
    sudo tee /etc/systemd/system/shard-api.service > /dev/null << 'EOF'
[Unit]
Description=Shard Oracle API
Requires=shard-daemon.service
After=shard-daemon.service

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_USER
WorkingDirectory=$APP_DIR/python
Environment=\"PATH=$PYTHON_VENV/bin\"
Environment=\"SHARD_API_KEYS=prod-key-change-me\"
Environment=\"SHARD_RATE_LIMIT_PER_MINUTE=120\"
Environment=\"SHARD_RUST_URL=http://127.0.0.1:9091\"
ExecStart=$PYTHON_VENV/bin/python run.py --port 8000
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
"

# ─────────────────────────────────────────────────────────────
# Step 8: Start services
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[8/8] Starting services...${NC}"
ssh -i "$SSH_KEY" ubuntu@$EC2_HOST "
    sudo systemctl enable shard-daemon
    sudo systemctl enable shard-api
    sudo systemctl start shard-daemon
    sleep 2
    sudo systemctl start shard-api
    
    # Verify services are running
    echo '=== Service Status ==='
    sudo systemctl status shard-daemon --no-pager
    sudo systemctl status shard-api --no-pager
    
    # Test health endpoints
    echo ''
    echo '=== Health Check ==='
    curl -s http://localhost:8000/health | head -c 200
    echo ''
"

echo -e "${GREEN}=== Deployment Complete! ===${NC}"
echo ""
echo "Services should now be running:"
echo "  - API:     http://$EC2_HOST:8000"
echo "  - Dashboard: http://$EC2_HOST:3000"
echo "  - Health:  http://$EC2_HOST:8000/health"
echo ""
echo "Next steps:"
echo "  1. Configure SSL certificate (Let's Encrypt recommended)"
echo "  2. Update SHARD_API_KEYS in the service file"
echo "  3. Set up bootstrap peers for P2P network"
