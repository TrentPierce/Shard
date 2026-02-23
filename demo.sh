#!/bin/bash
#
# Shard Local Demo
# 
# Starts a local Shard network with:
# - Rust daemon (verifier) + C++ engine
# - Next.js web app (Scout UI)
# - Downloaded GGUF model
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
}

# Download model if missing
check_models() {
    MODEL_DIR="models"
    MODEL_NAME="tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
    MODEL_PATH="$MODEL_DIR/$MODEL_NAME"
    
    mkdir -p "$MODEL_DIR"
    
    if [ ! -f "$MODEL_PATH" ]; then
        echo -e "${YELLOW}Downloading model for local demo (TinyLlama-1.1B)...${NC}"
        URL="https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf"
        
        if command -v wget &> /dev/null; then
            wget -O "$MODEL_PATH" "$URL"
        elif command -v curl &> /dev/null; then
            curl -L -o "$MODEL_PATH" "$URL"
        else
            echo -e "${RED}Error: Neither wget nor curl found. Please download model manually to $MODEL_PATH${NC}"
            exit 1
        fi
        echo -e "${GREEN}Model downloaded.${NC}"
    else
        echo -e "${GREEN}Model found: $MODEL_NAME${NC}"
    fi
}

# Start services
start_services() {
    echo -e "\n${GREEN}[1/2] Starting services via Docker Compose...${NC}"
    
    docker-compose up -d --build --remove-orphans
    
    # Wait for health check
    echo -e "Waiting for daemon to be healthy..."
    for i in {1..60}; do
        if docker-compose exec -T shard-daemon wget -q --spider http://localhost:9091/health; then
            echo -e "${GREEN}✓ Daemon is healthy!${NC}"
            return 0
        fi
        echo -n "."
        sleep 2
    done
    
    echo -e "\n${RED}✗ Daemon failed to become healthy${NC}"
    docker-compose logs shard-daemon
    exit 1
}

# Open browser
open_browser() {
    echo -e "\n${GREEN}[2/2] Opening demo in browser...${NC}"
    
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
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

# Cleanup on exit
cleanup() {
    # Don't shut down docker automatically, keep it running for the user
    echo -e "${GREEN}Demo is running in background. Use 'docker-compose down' to stop.${NC}"
}

# Main execution
main() {
    check_prereq
    check_models
    start_services
    open_browser
    print_info
}

main "$@"
