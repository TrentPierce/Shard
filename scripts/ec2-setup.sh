#!/bin/bash
# Shard Network - EC2 Setup (run as ubuntu user, NOT sudo)

echo "=== Shard EC2 Setup ==="

# 1. Clone repo (as ubuntu user, not sudo)
cd /opt
rm -rf Shard
git clone https://github.com/TrentPierce/Shard.git
cd Shard

# 2. Create venv
python3 -m venv venv
source venv/bin/activate

# 3. Install Python deps
pip install --upgrade pip
pip install -r desktop/python/requirements.txt

# 4. Build web
cd web
npm install
npm run build

# 5. Start API
cd ../desktop/python
SHARD_API_KEYS=test-key SHARD_RATE_LIMIT_PER_MINUTE=120 nohup python run.py --port 8000 > /tmp/api.log 2>&1 &

# 6. Start Web
cd ../../web
nohup npm start > /tmp/web.log 2>&1 &

# 7. Wait and check
sleep 10
echo "=== Checking services ==="
curl -s http://localhost:8000/health || echo "API not running - check /tmp/api.log"
curl -s http://localhost:3000 | head -c 100 || echo "Web not running - check /tmp/web.log"
