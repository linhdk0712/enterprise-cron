#!/bin/bash
# Test script to verify worker fix

set -e

echo "=== Testing Worker Fix ==="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 1. Check if workers are running
echo "1. Checking worker containers..."
WORKER_COUNT=$(docker ps | grep -c "worker" || true)
if [ "$WORKER_COUNT" -ge 1 ]; then
    echo -e "${GREEN}✓ Workers are running ($WORKER_COUNT instances)${NC}"
else
    echo -e "${RED}✗ No workers running!${NC}"
    exit 1
fi
echo ""

# 2. Get job ID
echo "2. Getting job ID..."
JOB_ID=$(docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron -t -c "SELECT id FROM jobs LIMIT 1;" | xargs)
if [ -z "$JOB_ID" ]; then
    echo -e "${RED}✗ No jobs found in database!${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Job ID: $JOB_ID${NC}"
echo ""

# 3. Delete old pending executions
echo "3. Cleaning up old pending executions..."
docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron -c "DELETE FROM job_executions WHERE status = 'pending';" > /dev/null
echo -e "${GREEN}✓ Old executions cleaned${NC}"
echo ""

# 4. Check worker logs (last 10 lines)
echo "4. Recent worker logs:"
echo -e "${YELLOW}---${NC}"
docker logs rust-enterprise-cron-worker-1 --tail 10 2>&1 | grep -E "Starting|Consumer|Processing|Loaded|Executing" || echo "No relevant logs yet"
echo -e "${YELLOW}---${NC}"
echo ""

# 5. Wait for scheduler to trigger job
echo "5. Waiting for scheduler to trigger job (max 15 seconds)..."
for i in {1..15}; do
    EXEC_COUNT=$(docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron -t -c "SELECT COUNT(*) FROM job_executions WHERE job_id = '$JOB_ID' AND created_at > NOW() - INTERVAL '20 seconds';" | xargs)
    if [ "$EXEC_COUNT" -gt 0 ]; then
        echo -e "${GREEN}✓ New execution created!${NC}"
        break
    fi
    echo -n "."
    sleep 1
done
echo ""

# 6. Check execution status
echo "6. Checking execution status..."
docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron -c "
SELECT 
    id, 
    status, 
    started_at, 
    completed_at,
    CASE 
        WHEN completed_at IS NOT NULL THEN 
            EXTRACT(EPOCH FROM (completed_at - started_at)) || 's'
        ELSE 'N/A'
    END as duration
FROM job_executions 
WHERE job_id = '$JOB_ID'
ORDER BY created_at DESC 
LIMIT 3;
"
echo ""

# 7. Check worker logs for processing
echo "7. Checking worker logs for job processing..."
echo -e "${YELLOW}---${NC}"
docker logs rust-enterprise-cron-worker-1 --since 30s 2>&1 | grep -E "Processing job|Loaded job|Executing step|completed successfully|failed" | tail -20 || echo "No processing logs found"
echo -e "${YELLOW}---${NC}"
echo ""

# 8. Summary
echo "=== Summary ==="
LATEST_STATUS=$(docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron -t -c "SELECT status FROM job_executions WHERE job_id = '$JOB_ID' ORDER BY created_at DESC LIMIT 1;" | xargs)

if [ "$LATEST_STATUS" = "success" ]; then
    echo -e "${GREEN}✓ SUCCESS: Job executed successfully!${NC}"
    exit 0
elif [ "$LATEST_STATUS" = "running" ]; then
    echo -e "${YELLOW}⚠ RUNNING: Job is still running...${NC}"
    echo "Check logs: docker logs rust-enterprise-cron-worker-1 -f"
    exit 0
elif [ "$LATEST_STATUS" = "failed" ]; then
    echo -e "${RED}✗ FAILED: Job execution failed${NC}"
    echo "Check error:"
    docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron -c "SELECT error FROM job_executions WHERE job_id = '$JOB_ID' ORDER BY created_at DESC LIMIT 1;"
    exit 1
elif [ "$LATEST_STATUS" = "pending" ]; then
    echo -e "${RED}✗ PENDING: Job still pending (worker not processing!)${NC}"
    echo "Worker may not be consuming messages correctly"
    exit 1
else
    echo -e "${YELLOW}⚠ UNKNOWN: Status = $LATEST_STATUS${NC}"
    exit 1
fi
