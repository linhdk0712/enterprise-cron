#!/bin/bash

# Vietnam Enterprise Cron System - Service Status Check
# This script checks the status of all running services

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 Vietnam Enterprise Cron System - Service Status${NC}"
echo ""

# Check infrastructure services (Docker)
echo -e "${BLUE}📦 Infrastructure Services (Docker):${NC}"
echo ""

check_docker_service() {
    local service=$1
    local port=$2
    local name=$3
    
    if docker ps --filter "name=$service" --format "{{.Status}}" | grep -q "Up"; then
        echo -e "  ${GREEN}✓${NC} $name (port $port) - Running"
        return 0
    else
        echo -e "  ${RED}✗${NC} $name (port $port) - Not running"
        return 1
    fi
}

check_docker_service "vietnam-cron-postgres" "5432" "PostgreSQL"
check_docker_service "vietnam-cron-redis" "6379" "Redis"
check_docker_service "vietnam-cron-nats" "4222" "NATS"
check_docker_service "vietnam-cron-minio" "9000" "MinIO"

echo ""
echo -e "${BLUE}🚀 Application Services (Local):${NC}"
echo ""

# Check local services
check_local_service() {
    local port=$1
    local name=$2
    local endpoint=$3
    
    if curl -s -f "$endpoint" > /dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} $name (port $port) - Running"
        return 0
    else
        echo -e "  ${RED}✗${NC} $name (port $port) - Not responding"
        return 1
    fi
}

# Check if processes are running
check_process() {
    local name=$1
    local pattern=$2
    
    if pgrep -f "$pattern" > /dev/null 2>&1; then
        echo -e "  ${GREEN}✓${NC} $name - Process running"
        return 0
    else
        echo -e "  ${RED}✗${NC} $name - Process not found"
        return 1
    fi
}

check_process "Scheduler" "./target/release/scheduler"
check_process "Worker" "./target/release/worker"

# Check API with health endpoint
if curl -s -f http://localhost:8080/health > /dev/null 2>&1; then
    echo -e "  ${GREEN}✓${NC} API (port 8080) - Running and healthy"
else
    echo -e "  ${RED}✗${NC} API (port 8080) - Not responding"
fi

# Check metrics endpoint
if curl -s http://localhost:9090/metrics | head -1 > /dev/null 2>&1; then
    echo -e "  ${GREEN}✓${NC} Metrics (port 9090) - Available"
else
    echo -e "  ${YELLOW}⚠${NC} Metrics (port 9090) - Not available"
fi

echo ""
echo -e "${BLUE}📍 Access Points:${NC}"
echo "  • Dashboard:          http://localhost:8080"
echo "  • API:                http://localhost:8080/api"
echo "  • Health Check:       http://localhost:8080/health"
echo "  • Prometheus Metrics: http://localhost:9090/metrics"
echo "  • MinIO Console:      http://localhost:9001 (minioadmin/minioadmin)"
echo "  • NATS Monitoring:    http://localhost:8222"
echo ""
echo -e "${BLUE}📝 Useful Commands:${NC}"
echo "  • View scheduler logs: tail -f logs/scheduler.log (or check process output)"
echo "  • View worker logs:    tail -f logs/worker.log (or check process output)"
echo "  • View API logs:       tail -f logs/api.log (or check process output)"
echo "  • Stop all services:   ./stop-services.sh"
echo ""
