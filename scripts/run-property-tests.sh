#!/bin/bash
# Script to run property-based tests manually
# These tests require Redis to be running and take longer to execute

set -e

echo "=========================================="
echo "Running Property-Based Tests"
echo "=========================================="
echo ""
echo "⚠️  Prerequisites:"
echo "   - Redis must be running on localhost:6379"
echo "   - Tests will run 100+ iterations per property"
echo ""

# Check if Redis is running
if ! redis-cli ping > /dev/null 2>&1; then
    echo "❌ Error: Redis is not running!"
    echo ""
    echo "Please start Redis first:"
    echo "  - macOS: brew services start redis"
    echo "  - Linux: sudo systemctl start redis"
    echo "  - Docker: docker run -d -p 6379:6379 redis:7-alpine"
    echo ""
    exit 1
fi

echo "✅ Redis is running"
echo ""

# Run property tests with ignored flag
echo "Running property tests..."
echo ""

cargo test --package common --test property_tests -- --ignored --nocapture

echo ""
echo "=========================================="
echo "✅ Property tests completed!"
echo "=========================================="
