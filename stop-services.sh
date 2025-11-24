#!/bin/bash

# Vietnam Enterprise Cron System - Stop Services Script
# This script stops all running services (local and Docker)

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🛑 Stopping Vietnam Enterprise Cron System Services${NC}"
echo ""

# Stop local application services
echo -e "${BLUE}Stopping local application services...${NC}"

stop_process() {
    local name=$1
    local pattern=$2
    
    if pgrep -f "$pattern" > /dev/null 2>&1; then
        echo -n "  Stopping $name..."
        pkill -f "$pattern" || true
        sleep 1
        if pgrep -f "$pattern" > /dev/null 2>&1; then
            echo -e " ${YELLOW}⚠ Still running, force killing...${NC}"
            pkill -9 -f "$pattern" || true
        else
            echo -e " ${GREEN}✓${NC}"
        fi
    else
        echo -e "  $name - ${YELLOW}Not running${NC}"
    fi
}

stop_process "API" "./target/release/api"
stop_process "Worker" "./target/release/worker"
stop_process "Scheduler" "./target/release/scheduler"

echo ""
echo -e "${BLUE}Stopping Docker infrastructure services...${NC}"

# Stop Docker services (keep infrastructure running by default)
# Uncomment the line below to stop infrastructure services too
# docker-compose down

echo -e "  ${YELLOW}ℹ${NC}  Infrastructure services (PostgreSQL, Redis, NATS, MinIO) are still running"
echo -e "  ${YELLOW}ℹ${NC}  To stop them, run: docker-compose down"

echo ""
echo -e "${GREEN}✅ Application services stopped${NC}"
echo ""
