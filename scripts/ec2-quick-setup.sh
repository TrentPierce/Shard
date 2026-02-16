#!/bin/bash
# Shard Network - EC2 Quick Setup Script
# Run this on your EC2 instance after launching
#
# Usage: curl -sSL https://raw.githubusercontent.com/ShardNetwork/Shard/main/scripts/ec2-quick-setup.sh | bash
#
# OR copy/paste the commands below manually

set -e

echo "=== Shard Network EC2 Quick Setup ==="
echo ""

# ─────────────────────────────────────────────────────────────
# Step 1: Install system dependencies
# ─────────────────────────────────────────────────────────────
echo "[1/6] Installing system dependencies..."
sudo apt update
sudo apt install -y python3.11 python3.11-venv python3-pip git curl nginx

# ─────────────────────────────────────────────────────────────
# Step 2: Clone or update Shard repository
# ─────────────────────────────────────────────────────────────
echo "[2/6] Setting up Shard..."
cd /opt
if [ -d "Shard" ]; then
    echo "Updating existing Shard installation..."
    cd Shard
    git pull origin main
else
    echo "Cloning Shard repository..."
    sudo git clone https://github.com/TrentPierce/Shard.git
    cd Shard
fi

# ─────────────────────────────────────────────────────────────
# Step 3: Set up Python virtual environment
# ─────────────────────────────────────────────────────────────
echo "[3/6] Setting up Python environment..."
python3 -m venv venv
source venv/bin/activate
pip install --upgrade pip
pip install -r desktop/python/requirements.txt

# ─────────────────────────────────────────────────────────────
# Step 4: Set up Web dashboard
# ─────────────────────────────────────────────────────────────
echo "[4/6] Setting up Web dashboard..."
cd web
npm install
npm run build

# ─────────────────────────────────────────────────────────────
# Step 5: Configure Nginx
# ─────────────────────────────────────────────────────────────
echo "[5/6] Configuring Nginx..."
sudo cp /opt/Shard/docs/nginx-telemetry-dashboard.conf /etc/nginx/sites-available/shard
sudo ln -sf /etc/nginx/sites-available/shard /etc/nginx/sites-enabled/
sudo nginx -t

# ─────────────────────────────────────────────────────────────
# Step 6: Start services
# ─────────────────────────────────────────────────────────────
echo "[6/6] Starting services..."

# Start Python API in background
cd /opt/Shard/desktop/python
export SHARD_API_KEYS="prod-key-$(date +%s)"
export SHARD_RATE_LIMIT_PER_MINUTE=120
nohup python run.py --port 8000 > /tmp/shard-api.log 2>&1 &
echo "Python API started (check with: tail /tmp/shard-api.log)"

# Start Web Dashboard in background
cd /opt/Shard/web
nohup npm start > /tmp/shard-web.log 2>&1 &
echo "Web Dashboard started (check with: tail /tmp/shard-web.log)"

# Wait for services to start
sleep 5

echo ""
echo "=== Setup Complete! ==="
echo ""
echo "Services:"
echo "  - API:     http://$(curl -s ifconfig.me):8000"
echo "  - Dashboard: http://$(curl -s ifconfig.me):3000"
echo "  - Health:  http://$(curl -s ifconfig.me):8000/health"
echo ""
echo "Test with:"
echo "  curl http://localhost:8000/health"
echo "  curl http://localhost:3000"
echo ""
echo "Logs:"
echo "  tail -f /tmp/shard-api.log"
echo "  tail -f /tmp/shard-web.log"
