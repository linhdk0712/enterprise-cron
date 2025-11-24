#!/bin/bash

# Vietnam Enterprise Cron System - Log Checker
# This script helps you view logs from different services

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}📋 Vietnam Enterprise Cron System - Log Viewer${NC}"
echo ""

# Check if service name is provided
if [ -z "$1" ]; then
    echo "Usage: ./check-logs.sh [service] [options]"
    echo ""
    echo "Services:"
    echo "  • api        - API server logs"
    echo "  • scheduler  - Scheduler logs"
    echo "  • worker     - Worker logs"
    echo "  • postgres   - PostgreSQL logs"
    echo "  • redis      - Redis logs"
    echo "  • nats       - NATS logs"
    echo "  • minio      - MinIO logs"
    echo "  • all        - All services logs"
    echo ""
    echo "Options:"
    echo "  -f, --follow    Follow log output"
    echo "  --tail N        Show last N lines (default: 100)"
    echo ""
    echo "Examples:"
    echo "  ./check-logs.sh api -f"
    echo "  ./check-logs.sh worker --tail 50"
    echo "  ./check-logs.sh all"
    exit 1
fi

SERVICE=$1
shift

# Default options
FOLLOW=""
TAIL="--tail=100"

# Parse options
while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--follow)
            FOLLOW="-f"
            shift
            ;;
        --tail)
            TAIL="--tail=$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Show logs
if [ "$SERVICE" = "all" ]; then
    echo -e "${GREEN}Showing logs from all services...${NC}"
    docker-compose logs $FOLLOW $TAIL
else
    echo -e "${GREEN}Showing logs from $SERVICE...${NC}"
    docker-compose logs $FOLLOW $TAIL $SERVICE
fi
