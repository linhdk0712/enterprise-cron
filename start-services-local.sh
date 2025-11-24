#!/bin/bash

# Vietnam Enterprise Cron System - Start Services Locally
# This script starts all application services locally (not in Docker)

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Starting Vietnam Enterprise Cron System (Local Mode)${NC}"
echo ""

# Check if binaries exist
if [ ! -f "target/release/api" ] || [ ! -f "target/release/scheduler" ] || [ ! -f "target/release/worker" ]; then
    echo -e "${YELLOW}⚠ Binaries not found. Building...${NC}"
    cargo build --release --bin api --bin scheduler --bin worker
    echo -e "${GREEN}✓ Build completed${NC}"
    echo ""
fi

# Check if infrastructure services are running
echo -e "${BLUE}Checking infrastructure services...${NC}"

check_docker_service() {
    local service=$1
    local name=$2
    
    if docker ps --filter "name=$service" --format "{{.Status}}" | grep -q "Up"; then
        echo -e "  ${GREEN}✓${NC} $name is running"
        return 0
    else
        echo -e "  ${RED}✗${NC} $name is not running"
        return 1
    fi
}

all_infra_running=true
check_docker_service "vietnam-cron-postgres" "PostgreSQL" || all_infra_running=false
check_docker_service "vietnam-cron-redis" "Redis" || all_infra_running=false
check_docker_service "vietnam-cron-nats" "NATS" || all_infra_running=false
check_docker_service "vietnam-cron-minio" "MinIO" || all_infra_running=false

if [ "$all_infra_running" = false ]; then
    echo ""
    echo -e "${YELLOW}⚠ Some infrastructure services are not running${NC}"
    echo -e "${YELLOW}  Starting infrastructure services with Docker...${NC}"
    docker-compose up -d postgres redis nats minio
    echo -e "${GREEN}✓ Infrastructure services started${NC}"
    echo ""
    echo -e "${YELLOW}  Waiting for services to be healthy...${NC}"
    sleep 10
fi

echo ""
echo -e "${BLUE}Starting application services...${NC}"

# Create logs directory if it doesn't exist
mkdir -p logs

# Start Scheduler
echo -n "  Starting Scheduler..."
RUST_LOG=info nohup ./target/release/scheduler > logs/scheduler.log 2>&1 &
SCHEDULER_PID=$!
sleep 2
if ps -p $SCHEDULER_PID > /dev/null; then
    echo -e " ${GREEN}✓${NC} (PID: $SCHEDULER_PID)"
else
    echo -e " ${RED}✗ Failed to start${NC}"
    echo "  Check logs/scheduler.log for details"
fi

# Start Worker
echo -n "  Starting Worker..."
RUST_LOG=info nohup ./target/release/worker > logs/worker.log 2>&1 &
WORKER_PID=$!
sleep 2
if ps -p $WORKER_PID > /dev/null; then
    echo -e " ${GREEN}✓${NC} (PID: $WORKER_PID)"
else
    echo -e " ${RED}✗ Failed to start${NC}"
    echo "  Check logs/worker.log for details"
fi

# Start API
echo -n "  Starting API..."
RUST_LOG=info nohup ./target/release/api > logs/api.log 2>&1 &
API_PID=$!
sleep 3
if ps -p $API_PID > /dev/null; then
    echo -e " ${GREEN}✓${NC} (PID: $API_PID)"
else
    echo -e " ${RED}✗ Failed to start${NC}"
    echo "  Check logs/api.log for details"
fi

echo ""
echo -e "${BLUE}Verifying services...${NC}"
sleep 2

# Check API health
if curl -s -f http://localhost:8080/health > /dev/null 2>&1; then
    echo -e "  ${GREEN}✓${NC} API is responding"
else
    echo -e "  ${YELLOW}⚠${NC} API is not responding yet (may still be starting)"
fi

echo ""
echo -e "${GREEN}✅ Services started successfully!${NC}"
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
echo "  • Check status:       ./check-services.sh"
echo "  • View scheduler logs: tail -f logs/scheduler.log"
echo "  • View worker logs:    tail -f logs/worker.log"
echo "  • View API logs:       tail -f logs/api.log"
echo "  • Stop services:       ./stop-services.sh"
echo ""
echo -e "${BLUE}🔐 Default Login (Database Mode):${NC}"
echo "  • Username: admin"
echo "  • Password: admin123"
echo ""
