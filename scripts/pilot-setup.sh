#!/bin/bash
# Shard Pilot Setup Script
# Generates a configuration for a private enterprise pilot.

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Shard Enterprise Pilot Setup${NC}"
echo "============================"

# Gather info
read -p "Organization Name (e.g. AcmeCorp): " ORG_NAME
read -p "Server Public IP/Hostname (e.g. shard.acme.com): " SERVER_HOST
read -p "Expected Concurrent Users (e.g. 50): " USERS

# Sanitize Org Name for URL
ORG_SLUG=$(echo "$ORG_NAME" | tr '[:upper:]' '[:lower:]' | tr ' ' '-')

echo -e "\n${GREEN}Configuring for $ORG_NAME ($ORG_SLUG)...${NC}"

# Generate .env
cat > .env.pilot <<EOF
SHARD_ORG_NAME="$ORG_NAME"
SHARD_ORG_SLUG="$ORG_SLUG"
SHARD_PUBLIC_HOST="$SERVER_HOST"
SHARD_CAPACITY_TARGET="$USERS"
NEXT_PUBLIC_API_URL="http://$SERVER_HOST:9091"
NEXT_PUBLIC_RUST_URL="http://$SERVER_HOST:9091"
EOF

# Generate docker-compose.yml
cat > docker-compose.pilot.yml <<EOF
services:
  shard-daemon:
    image: ghcr.io/trentpierce/shard:latest
    container_name: shard-daemon
    command: ["shard-daemon", "--control-port", "9091", "--tcp-port", "4001", "--public-host", "$SERVER_HOST", "--public-api", "true"]
    ports:
      - "9091:9091"
      - "4001:4001"
      - "9090:9090/udp"
      - "9092:9092/udp"
    env_file: .env.pilot
    volumes:
      - shard-data:/root/.local/share/shard
      - shard-models:/root/.cache/shard/models
    restart: always

  web:
    image: ghcr.io/trentpierce/shard-web:latest
    container_name: shard-web
    ports:
      - "3000:3000"
    env_file: .env.pilot
    environment:
      - NEXT_PUBLIC_ORG_NAME=$ORG_NAME
    depends_on:
      - shard-daemon
    restart: always

volumes:
  shard-data:
  shard-models:
EOF

echo -e "\n${GREEN}Setup Complete!${NC}"
echo "------------------------------------------------"
echo "1. Start the pilot network:"
echo "   docker-compose -f docker-compose.pilot.yml up -d"
echo ""
echo "2. Share this URL with your users to add compute:"
echo "   http://$SERVER_HOST:3000/join/$ORG_SLUG"
echo ""
echo "3. View your admin dashboard:"
echo "   http://$SERVER_HOST:3000/admin"
echo "------------------------------------------------"
