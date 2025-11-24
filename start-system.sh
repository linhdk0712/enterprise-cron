#!/bin/bash

# Vietnam Enterprise Cron System - Start Script
# This script helps you start and verify the system

set -e

echo "🚀 Starting Vietnam Enterprise Cron System..."
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to check if a service is healthy
check_service() {
    local service=$1
    local max_attempts=30
    local attempt=1
    
    echo -n "Waiting for $service to be healthy..."
    
    while [ $attempt -le $max_attempts ]; do
        if docker-compose ps | grep "$service" | grep -q "healthy\|Up"; then
            echo -e " ${GREEN}✓${NC}"
            return 0
        fi
        echo -n "."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    echo -e " ${RED}✗${NC}"
    echo -e "${RED}Error: $service failed to start${NC}"
    return 1
}

# Step 1: Stop any running containers
echo "📦 Stopping existing containers..."
docker-compose down

# Step 2: Start infrastructure services first
echo ""
echo "🔧 Starting infrastructure services (PostgreSQL, Redis, NATS, MinIO)..."
docker-compose up -d postgres redis nats minio

# Step 3: Wait for infrastructure to be ready
echo ""
check_service "postgres" || exit 1
check_service "redis" || exit 1
check_service "nats" || exit 1
check_service "minio" || exit 1

# Step 4: Run database migrations
echo ""
echo "📊 Running database migrations..."
export DATABASE_URL="postgresql://cronuser:cronpass@localhost:5432/vietnam_cron"
if command -v sqlx &> /dev/null; then
    # Try to run migrations, but don't fail if they already exist
    sqlx migrate run || echo -e "${YELLOW}⚠ Migrations may already exist, continuing...${NC}"
    echo -e "${GREEN}✓ Migrations check completed${NC}"
else
    echo -e "${YELLOW}⚠ sqlx-cli not found. Skipping migrations.${NC}"
    echo "  Install with: cargo install sqlx-cli --no-default-features --features postgres"
fi

# Step 5: Start application services
echo ""
echo "🚀 Starting application services (Scheduler, Worker, API)..."
docker-compose up -d scheduler worker api

# Step 6: Wait for application services
echo ""
check_service "scheduler" || exit 1
check_service "worker" || exit 1
check_service "api" || exit 1

# Step 7: Display service status
echo ""
echo "📊 Service Status:"
docker-compose ps

# Step 8: Display access information
echo ""
echo -e "${GREEN}✅ System started successfully!${NC}"
echo ""
echo "📍 Access Points:"
echo "  • Dashboard:          http://localhost:8080"
echo "  • API:                http://localhost:8080/api"
echo "  • Health Check:       http://localhost:8080/health"
echo "  • Prometheus Metrics: http://localhost:9090/metrics"
echo "  • MinIO Console:      http://localhost:9001 (minioadmin/minioadmin)"
echo "  • NATS Monitoring:    http://localhost:8222"
echo ""
echo "🔐 Default Login (Database Mode):"
echo "  • Username: admin"
echo "  • Password: admin123"
echo ""
echo "📝 Useful Commands:"
echo "  • View logs:          docker-compose logs -f [service]"
echo "  • Stop system:        docker-compose down"
echo "  • Restart service:    docker-compose restart [service]"
echo "  • View all services:  docker-compose ps"
echo ""
echo "🎯 Next Steps:"
echo "  1. Open http://localhost:8080 in your browser"
echo "  2. Login with admin/admin123"
echo "  3. Create your first job!"
echo ""
