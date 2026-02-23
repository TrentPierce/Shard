#!/usr/bin/env bash
#
# Shard Pilot Setup Script
# Creates a self-hosted private Shard network for enterprise pilots
# 
# Usage: ./pilot-setup.sh
#

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}"
echo "╔════════════════════════════════════════════════════════════╗"
echo "║         Shard Pilot Network Setup (Enterprise)             ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo "This script will set up a private Shard network for your organization."
echo "Estimated setup time: 5-10 minutes"
echo ""

# Check for required tools
command -v docker >/dev/null 2>&1 || { echo -e "${RED}Error: docker is required but not installed.${NC}" >&2; exit 1; }
command -v docker compose >/dev/null 2>&1 || { echo -e "${RED}Error: docker compose is required but not installed.${NC}" >&2; exit 1; }

# Function to prompt for input with default
prompt() {
    local prompt="$1"
    local default="$2"
    local var_name="$3"
    
    if [ -n "$default" ]; then
        read -p "$prompt [$default]: " value
        value=${value:-$default}
    else
        read -p "$prompt: " value
    fi
    
    # Trim whitespace
    value=$(echo "$value" | xargs)
    eval "$var_name=\$value"
}

# Function to validate org name (alphanumeric and hyphens only)
validate_org_name() {
    local name="$1"
    if [[ ! "$name" =~ ^[a-zA-Z0-9-]+$ ]]; then
        echo -e "${RED}Organization name must contain only letters, numbers, and hyphens.${NC}"
        return 1
    fi
    return 0
}

# Function to validate IP address
validate_ip() {
    local ip="$1"
    local regex="^([0-9]{1,3}\.){3}[0-9]{1,3}$"
    if [[ ! "$ip" =~ $regex ]]; then
        # Allow hostname (letters, numbers, dots, hyphens)
        if [[ ! "$ip" =~ ^[a-zA-Z0-9.-]+$ ]]; then
            echo -e "${RED}Invalid IP address or hostname.${NC}"
            return 1
        fi
    fi
    return 0
}

# Gather configuration
echo -e "${YELLOW}Step 1 of 3: Organization Details${NC}"
echo "-----------------------------------"

while true; do
    prompt "Organization name" "mycompany" ORG_NAME
    if validate_org_name "$ORG_NAME"; then
        break
    fi
done

prompt "Server IP or hostname" "localhost" SERVER_HOST
while ! validate_ip "$SERVER_HOST"; do
    prompt "Server IP or hostname" "$SERVER_HOST" SERVER_HOST
done

prompt "Expected number of users (Scouts)" "50" USER_COUNT
if ! [[ "$USER_COUNT" =~ ^[0-9]+$ ]]; then
    echo -e "${RED}User count must be a number.${NC}"
    USER_COUNT=50
fi

echo ""
echo -e "${YELLOW}Step 2 of 3: Network Configuration${NC}"
echo "-----------------------------------"

prompt "API port (default: 9091)" "9091" API_PORT
prompt "P2P port (default: 4001)" "4001" P2P_PORT

# Generate unique org ID
ORG_ID="${ORG_NAME}-$(date +%s)"

echo ""
echo -e "${YELLOW}Step 3 of 3: Generating Configuration${NC}"
echo "-----------------------------------"

# Create output directory
OUTPUT_DIR="$PWD/pilot-${ORG_NAME}"
mkdir -p "$OUTPUT_DIR"

# Generate .env file
cat > "$OUTPUT_DIR/.env" << EOF
# Shard Pilot Network Configuration
# Generated: $(date)
# Organization: $ORG_NAME

# Organization
SHARD_ORG_NAME=$ORG_NAME
SHARD_ORG_ID=$ORG_ID

# Network
SHARD_API_HOST=$SERVER_HOST
SHARD_API_PORT=$API_PORT
SHARD_P2P_PORT=$P2P_PORT

# Scout Configuration
SHARD_MAX_SCOUTS=$USER_COUNT
SHARD_SCOUT_TIMEOUT_MS=800

# Optional: Enable credit system
# SHARD_CREDIT_GATING_ENABLED=true

# Optional: Private mesh (exclude from public bootstrap)
# SHARD_PRIVATE_MESH=true
EOF

# Generate docker-compose.yml
cat > "$OUTPUT_DIR/docker-compose.yml" << EOF
# Shard Pilot Network - Auto-generated
# Organization: $ORG_NAME
# Generated: $(date)

services:
  shard-daemon:
    image: shard-daemon:latest
    build:
      context: ../desktop/rust
      dockerfile: ../desktop/rust/Dockerfile
    container_name: shard-pilot-${ORG_NAME}
    ports:
      - "${API_PORT}:9091"
      - "${P2P_PORT}:4001"
      - "9090:9090"  # WebRTC
      - "9092:9092"  # QUIC
    environment:
      - SHARD_ORG_NAME=${ORG_NAME}
      - SHARD_ORG_ID=${ORG_ID}
      - SHARD_MAX_SCOUTS=${USER_COUNT}
      - SHARD_SCOUT_TIMEOUT_MS=800
      - SHARD_ANNOUNCE_ADDR=/ip4/${SERVER_HOST}/tcp/${P2P_PORT}
    volumes:
      - shard-data:/data
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9091/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    networks:
      - shard-pilot

  # Optional: Metrics dashboard
  prometheus:
    image: prom/prometheus:latest
    container_name: shard-prometheus-${ORG_NAME}
    ports:
      - "9095:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    profiles:
      - monitoring
    networks:
      - shard-pilot

volumes:
  shard-data:
  prometheus-data:

networks:
  shard-pilot:
    driver: bridge
EOF

# Generate prometheus.yml for metrics
cat > "$OUTPUT_DIR/prometheus.yml" << EOF
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'shard-daemon'
    static_configs:
      - targets: ['shard-daemon:9091']
EOF

# Generate Scout join URL
JOIN_URL="http://${SERVER_HOST}:${API_PORT}/join/${ORG_NAME}"

# Create quick-start script
cat > "$OUTPUT_DIR/start.sh" << EOF
#!/usr/bin/env bash
# Shard Pilot Quick Start
# Organization: $ORG_NAME

echo "Starting Shard Pilot Network..."
echo "Organization: $ORG_NAME"
echo ""

cd "\$(dirname "\$0")"

# Start the daemon
docker compose up -d

echo ""
echo "Services starting..."
sleep 5

echo ""
echo "✓ Shard Pilot Network is running!"
echo ""
echo "  API Endpoint:    http://${SERVER_HOST}:${API_PORT}"
echo "  Join URL:        ${JOIN_URL}"
echo "  Dashboard:       http://${SERVER_HOST}:${API_PORT}/admin"
echo "  Health Check:    http://${SERVER_HOST}:${API_PORT}/health"
echo ""
echo "To stop: docker compose down"
EOF

chmod +x "$OUTPUT_DIR/start.sh"

echo ""
echo -e "${GREEN}✓ Configuration generated successfully!${NC}"
echo ""
echo "Output directory: $OUTPUT_DIR"
echo ""
echo "Contents:"
echo "  - docker-compose.yml  (Docker services)"
echo "  - .env                (Environment configuration)"
echo "  - prometheus.yml      (Metrics configuration)"
echo "  - start.sh            (Quick start script)"
echo ""

echo -e "${YELLOW}Next steps:${NC}"
echo "-----------------------------------"
echo "1. Review the generated configuration in $OUTPUT_DIR"
echo "2. Run: cd $OUTPUT_DIR && ./start.sh"
echo "3. Share this URL with your team:"
echo -e "   ${GREEN}$JOIN_URL${NC}"
echo ""
echo "After starting, users can visit the join URL to become Scouts."
echo "The admin dashboard will be available at:"
echo -e "   ${GREEN}http://${SERVER_HOST}:${API_PORT}/admin${NC}"
echo ""
