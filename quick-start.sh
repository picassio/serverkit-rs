#!/usr/bin/env bash
#
# ServerKit - All-in-One Local Quick Start
#
# This script automates:
# 1. System dependency checks
# 2. Python venv & dependency setup
# 3. Frontend npm installation
# 4. Environment configuration (.env)
# 5. Launching both Backend & Frontend
#

set -euo pipefail

# Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${CYAN}🚀 Starting ServerKit Local Setup...${NC}"

# 1. Check for required system tools
check_tool() {
    if ! command -v "$1" &>/dev/null; then
        echo -e "${RED}✗ $1 is not installed.${NC}"
        echo -e "${YELLOW}Please run: sudo apt update && sudo apt install -y $2${NC}"
        exit 1
    fi
}

check_tool "python3" "python3 python3-venv python3-pip"
check_tool "node" "nodejs"
check_tool "npm" "npm"

# 2. Setup Backend
echo -e "\n${CYAN}📦 Setting up Backend...${NC}"
if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
fi

source venv/bin/activate
echo "Installing Python dependencies..."
pip install -r backend/requirements.txt --quiet
pip install gunicorn gevent gevent-websocket --quiet

# 3. Setup Frontend
echo -e "\n${CYAN}📦 Setting up Frontend...${NC}"
cd frontend
if [ ! -d "node_modules" ]; then
    echo "Installing Node modules (this may take a minute)..."
    npm install --silent
else
    echo "Node modules already installed."
fi
cd ..

# 4. Configuration
echo -e "\n${CYAN}⚙️  Configuring Environment...${NC}"
if [ ! -f ".env" ]; then
    echo "Creating .env from .env.example..."
    cp .env.example .env
    # Generate random secrets for dev
    sed -i "s/SECRET_KEY=.*/SECRET_KEY=$(openssl rand -hex 16)/" .env
    sed -i "s/JWT_SECRET_KEY=.*/JWT_SECRET_KEY=$(openssl rand -hex 16)/" .env
else
    echo ".env file already exists."
fi

# 5. Launch
echo -e "\n${GREEN}✅ Setup Complete!${NC}"
echo -e "Starting ServerKit..."
echo -e "  - ${CYAN}Frontend:${NC} http://localhost:41921"
echo -e "  - ${CYAN}Backend:${NC}  http://localhost:47927"
echo -e "\n${YELLOW}Press Ctrl+C to stop both servers.${NC}\n"

# Run dev.sh to handle the dual-process execution
chmod +x dev.sh
./dev.sh
