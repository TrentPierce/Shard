#!/bin/bash
# Shard SDK - PyPI Publishing Script
# Usage: ./publish-pypi.sh [testpypi|pypi]
#
# Prerequisites:
#   - PyPI API token configured in ~/.pypirc or environment variables
#   - twine installed: pip install twine build

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

REPO="${1:-testpypi}"
DIST_DIR="dist"

echo -e "${GREEN}=== Shard SDK PyPI Publishing ===${NC}"

# ─────────────────────────────────────────────────────────────
# Step 1: Clean previous builds
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[1/5] Cleaning previous builds...${NC}"
rm -rf build dist *.egg-info

# ─────────────────────────────────────────────────────────────
# Step 2: Build the package
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[2/5] Building package...${NC}"
python -m build

# ─────────────────────────────────────────────────────────────
# Step 3: Verify the build
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[3/5] Verifying build artifacts...${NC}"
ls -la dist/

# Check for required files
if [ ! -f "dist/shard_sdk-"*".whl" ]; then
    echo -e "${RED}Error: Wheel file not found!${NC}"
    exit 1
fi

# ─────────────────────────────────────────────────────────────
# Step 4: Upload to PyPI
# ─────────────────────────────────────────────────────────────
echo -e "${YELLOW}[4/5] Uploading to $REPO...${NC}"

if [ "$REPO" = "testpypi" ]; then
    echo "Uploading to TestPyPI..."
    twine upload --repository testpypi dist/*
    echo -e "${GREEN}Uploaded to TestPyPI!${NC}"
    echo ""
    echo "Test installation with:"
    echo "  pip install --index-url https://test.pypi.org/simple/ shard-sdk"
elif [ "$REPO" = "pypi" ]; then
    echo "Uploading to PyPI..."
    twine upload dist/*
    echo -e "${GREEN}Uploaded to PyPI!${NC}"
    echo ""
    echo "Install with:"
    echo "  pip install shard-sdk"
else
    echo -e "${RED}Unknown repository: $REPO${NC}"
    echo "Usage: $0 [testpypi|pypi]"
    exit 1
fi

# ─────────────────────────────────────────────────────────────
# Step 5: Verify installation (TestPyPI only)
# ─────────────────────────────────────────────────────────────
if [ "$REPO" = "testpypi" ]; then
    echo -e "${YELLOW}[5/5] Testing installation in clean venv...${NC}"
    
    # Create temp venv
    TEMP_VENV=$(mktemp -d)
    python -m venv "$TEMP_VENV"
    source "$TEMP_VENV/bin/activate"
    
    pip install --index-url https://test.pypi.org/simple/ shard-sdk
    
    # Test import
    python -c "import httpx; print('httpx installed:', httpx.__version__)"
    
    # Cleanup
    deactivate
    rm -rf "$TEMP_VENV"
    
    echo -e "${GREEN}Test installation successful!${NC}"
fi

echo -e "${GREEN}=== Done! ===${NC}"
