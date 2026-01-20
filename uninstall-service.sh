#!/bin/bash
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}=== Crypto Trading Bot - Uninstall ===${NC}"

if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Please run with sudo: sudo ./uninstall-service.sh${NC}"
    exit 1
fi

echo -e "${YELLOW}[1/4] Stopping service...${NC}"
systemctl stop crypto-trading-bot.service 2>/dev/null || true
echo -e "${GREEN}✓ Service stopped${NC}"

echo -e "${YELLOW}[2/4] Disabling service...${NC}"
systemctl disable crypto-trading-bot.service 2>/dev/null || true
echo -e "${GREEN}✓ Service disabled${NC}"

echo -e "${YELLOW}[3/4] Removing service file...${NC}"
rm -f /etc/systemd/system/crypto-trading-bot.service
echo -e "${GREEN}✓ Service file removed${NC}"

echo -e "${YELLOW}[4/4] Reloading systemd daemon...${NC}"
systemctl daemon-reload
echo -e "${GREEN}✓ Daemon reloaded${NC}"

echo ""
echo -e "${GREEN}=== Uninstall Complete ===${NC}"
