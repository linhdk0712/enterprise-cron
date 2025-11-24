#!/bin/bash

# Test if Docker binaries can run

echo "Testing scheduler binary..."
timeout 5 docker run --rm \
  --network rust-enterprise-cron_cron-network \
  -e RUST_LOG=info \
  -e APP_DATABASE__URL=postgresql://cronuser:cronpass@postgres:5432/vietnam_cron \
  -e APP_REDIS__URL=redis://:redispass@redis:6379 \
  -e APP_NATS__URL=nats://nats:4222 \
  vietnam-cron:latest scheduler 2>&1 | tee /tmp/scheduler-test.log

echo ""
echo "Scheduler test output saved to /tmp/scheduler-test.log"
echo ""

echo "Testing API binary..."
timeout 5 docker run --rm \
  --network rust-enterprise-cron_cron-network \
  -e RUST_LOG=info \
  -e APP_DATABASE__URL=postgresql://cronuser:cronpass@postgres:5432/vietnam_cron \
  -e APP_REDIS__URL=redis://:redispass@redis:6379 \
  -e APP_NATS__URL=nats://nats:4222 \
  -e APP_MINIO__ENDPOINT=minio:9000 \
  -e APP_MINIO__ACCESS_KEY=minioadmin \
  -e APP_MINIO__SECRET_KEY=minioadmin \
  -e APP_MINIO__BUCKET=vietnam-cron \
  -e APP_MINIO__REGION=us-east-1 \
  vietnam-cron:latest api 2>&1 | tee /tmp/api-test.log

echo ""
echo "API test output saved to /tmp/api-test.log"
