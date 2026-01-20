#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}=== Crypto Trading Bot - Systemd Installation ===${NC}"

# Check if running as root for systemd operations
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Please run with sudo: sudo ./install-service.sh${NC}"
    exit 1
fi

# Auto-detect user (the user who ran sudo, not root)
INSTALL_USER="${SUDO_USER:-$(whoami)}"
INSTALL_GROUP="$INSTALL_USER"
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
SERVICE_FILE="crypto-trading-bot.service"

echo -e "${GREEN}Installing for user: $INSTALL_USER${NC}"
echo -e "${GREEN}Project directory: $PROJECT_DIR${NC}"

# Step 1: Build release binary
echo -e "${YELLOW}[1/6] Building release binary...${NC}"
cd "$PROJECT_DIR"
sudo -u "$INSTALL_USER" bash -c 'source "$HOME/.cargo/env" && cargo build --release'
echo -e "${GREEN}✓ Build complete${NC}"

# Step 2: Check if .env exists
echo -e "${YELLOW}[2/6] Checking configuration...${NC}"
if [ ! -f "$PROJECT_DIR/.env" ]; then
    echo -e "${RED}✗ .env file not found!${NC}"
    echo -e "${YELLOW}Creating .env from .env.example...${NC}"
    cp "$PROJECT_DIR/.env.example" "$PROJECT_DIR/.env"
    chown "$INSTALL_USER:$INSTALL_GROUP" "$PROJECT_DIR/.env"
    chmod 600 "$PROJECT_DIR/.env"
    echo -e "${YELLOW}⚠ Please edit .env with your API credentials before starting the service${NC}"
else
    echo -e "${GREEN}✓ .env file exists${NC}"
fi

# Step 3: Update service file with current user and paths
echo -e "${YELLOW}[3/6] Configuring service file for user $INSTALL_USER...${NC}"
TEMP_SERVICE="/tmp/$SERVICE_FILE"
cp "$PROJECT_DIR/$SERVICE_FILE" "$TEMP_SERVICE"
sed -i "s/User=machado/User=$INSTALL_USER/g" "$TEMP_SERVICE"
sed -i "s/Group=machado/Group=$INSTALL_GROUP/g" "$TEMP_SERVICE"
echo -e "${GREEN}✓ Service configured for user $INSTALL_USER${NC}"

# Step 4: Copy service file to systemd
echo -e "${YELLOW}[4/6] Installing systemd service...${NC}"
cp "$TEMP_SERVICE" /etc/systemd/system/
chmod 644 /etc/systemd/system/$SERVICE_FILE
rm -f "$TEMP_SERVICE"
echo -e "${GREEN}✓ Service file installed${NC}"

# Step 5: Reload systemd daemon
echo -e "${YELLOW}[5/6] Reloading systemd daemon...${NC}"
systemctl daemon-reload
echo -e "${GREEN}✓ Daemon reloaded${NC}"

# Step 6: Enable service (optional auto-start on boot)
echo -e "${YELLOW}[6/6] Enabling service...${NC}"
systemctl enable crypto-trading-bot.service
echo -e "${GREEN}✓ Service enabled${NC}"

echo ""
echo -e "${GREEN}=== Installation Complete ===${NC}"
echo ""
echo "Commands:"
echo "  Start:   sudo systemctl start crypto-trading-bot"
echo "  Stop:    sudo systemctl stop crypto-trading-bot"
echo "  Status:  sudo systemctl status crypto-trading-bot"
echo "  Logs:    journalctl -u crypto-trading-bot -f"
echo "  Logs (last 100 lines): journalctl -u crypto-trading-bot -n 100"
echo ""
echo -e "${YELLOW}⚠ Remember to configure your .env file before starting!${NC}"
