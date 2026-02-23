#!/bin/bash
#
# Shard Local Demo
# 
# Starts a local Shard network with:
# - Rust daemon (verifier)
# - Next.js web app (Scout UI)
# - Opens browser to start the demo
#
# Usage: ./demo.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  🚀 Shard Local Demo Starting...${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"

# Check prerequisites
check_prereq() {
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}Error: Docker is not installed${NC}"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        echo -e "${RED}Error: Docker Compose is not installed${NC}"
        exit 1
    fi
    
    if ! command -v npm &> /dev/null; then
        echo -e "${RED}Error: npm is not installed${NC}"
        exit 1
    fi
}

# Check if model files exist
check_models() {
    MODEL_DIR="${HOME}/.cache/shard/models"
    if [ ! -d "$MODEL_DIR" ]; then
        echo -e "${YELLOW}Note: No models found in $MODEL_DIR${NC}"
        echo -e "${YELLOW}The daemon will download models on first run.${NC}"
    fi
}

# Start the daemon
start_daemon() {
    echo -e "\n${GREEN}[1/3] Starting Shard daemon...${NC}"
    
    # Check if already running
    if docker ps --format '{{.Names}}' | grep -q "shard-daemon"; then
        echo -e "${YELLOW}Daemon already running. Stopping existing instance...${NC}"
        docker stop shard-daemon 2>/dev/null || true
    fi
    
    # Start daemon with docker-compose (without monitoring profile)
    docker-compose up -d shard-daemon
    
    # Wait for health check
    echo -e "Waiting for daemon to be healthy..."
    for i in {1..30}; do
        if curl -s http://localhost:9091/health > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Daemon is healthy!${NC}"
            return 0
        fi
        sleep 2
    done
    
    echo -e "${RED}✗ Daemon failed to become healthy${NC}"
    docker-compose logs shard-daemon
    exit 1
}

# Start the web app
start_web() {
    echo -e "\n${GREEN}[2/3] Starting web app...${NC}"
    
    cd web
    
    # Check if node_modules exists
    if [ ! -d "node_modules" ]; then
        echo "Installing dependencies..."
        npm install
    fi
    
    # Start Next.js in background
    npm run dev &
    WEB_PID=$!
    
    # Wait for web to be ready
    echo "Waiting for web app to start..."
    for i in {1..30}; do
        if curl -s http://localhost:3000 > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Web app is running!${NC}"
            cd ..
            return 0
        fi
        sleep 2
    done
    
    echo -e "${RED}✗ Web app failed to start${NC}"
    cd ..
    exit 1
}

# Open browser
open_browser() {
    echo -e "\n${GREEN}[3/3] Opening demo in browser...${NC}"
    
    if command -v xdg-open &> /dev/null; then
        xdg-open http://localhost:3000
    elif command -v open &> /dev/null; then
        open http://localhost:3000
    elif command -v start &> /dev/null; then
        start http://localhost:3000
    else
        echo -e "${YELLOW}Could not auto-open browser. Please visit:${NC}"
        echo -e "${BLUE}  http://localhost:3000${NC}"
    fi
}

# Print usage info
print_info() {
    echo -e "\n${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  📋 Demo Information${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "  • Web UI:        ${GREEN}http://localhost:3000${NC}"
    echo -e "  • Daemon API:    ${GREEN}http://localhost:9091${NC}"
    echo -e "  • Health:        ${GREEN}http://localhost:9091/health${NC}"
    echo -e "  • Metrics:       ${GREEN}http://localhost:9091/metrics${NC}"
    echo ""
    echo -e "${YELLOW}How to test:${NC}"
    echo -e "  1. Open http://localhost:3000 in your browser"
    echo -e "  2. Enable Scout mode (browser will load WebGPU model)"
    echo -e "  3. Type a prompt and watch tokens stream"
    echo -e "  4. Tokens should stream faster with Scout contribution!"
    echo ""
    echo -e "${YELLOW}To stop:${NC}"
    echo -e "  ${GREEN}docker-compose down${NC}"
    echo -e "  (Web app will stop when you close this terminal)"
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

# Cleanup on exit
cleanup() {
    echo -e "\n${YELLOW}Shutting down demo...${NC}"
    cd ..
    docker-compose down
    kill %1 2>/dev/null || true
    echo -e "${GREEN}Demo stopped.${NC}"
}

# Main execution
main() {
    check_prereq
    check_models
    
    # Trap cleanup
    trap cleanup EXIT
    
    start_daemon
    start_web
    open_browser
    print_info
    
    echo -e "${GREEN}🎉 Demo is running! Press Ctrl+C to stop.${NC}"
    
    # Wait indefinitely (Ctrl+C triggers cleanup)
    wait
}

main "$@"
