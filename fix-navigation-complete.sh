#!/bin/bash

echo "========================================="
echo "Navigation Fix - Complete Solution"
echo "========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Step 1: Verify template file exists${NC}"
if [ -f "api/templates/_job_details_content.html" ]; then
    echo -e "${GREEN}✓ Template file exists${NC}"
    echo "  Size: $(wc -c < api/templates/_job_details_content.html) bytes"
else
    echo -e "${RED}✗ Template file NOT found!${NC}"
    echo "  Expected: api/templates/_job_details_content.html"
    exit 1
fi

echo ""
echo -e "${YELLOW}Step 2: Check template has main-content wrapper${NC}"
if grep -q '<div id="main-content">' api/templates/_job_details_content.html; then
    echo -e "${GREEN}✓ Template has main-content wrapper${NC}"
else
    echo -e "${RED}✗ Template missing main-content wrapper!${NC}"
    exit 1
fi

echo ""
echo -e "${YELLOW}Step 3: Verify handler code${NC}"
if grep -q 'is_htmx = headers.get("HX-Request")' api/src/handlers/dashboard.rs; then
    echo -e "${GREEN}✓ Handler detects HTMX requests${NC}"
else
    echo -e "${RED}✗ Handler not detecting HTMX!${NC}"
    echo "  Check: job_details_partial function"
    exit 1
fi

if grep -q '_job_details_content.html' api/src/handlers/dashboard.rs; then
    echo -e "${GREEN}✓ Handler uses content template${NC}"
else
    echo -e "${RED}✗ Handler not using content template!${NC}"
    exit 1
fi

echo ""
echo -e "${YELLOW}Step 4: Check if server is running${NC}"
if curl -s http://localhost:8080/dashboard > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Server is running on port 8080${NC}"
else
    echo -e "${RED}✗ Server is NOT running!${NC}"
    echo ""
    echo "To start server:"
    echo "  cargo run --bin api"
    echo ""
    echo "Or with docker-compose:"
    echo "  docker-compose up api"
    exit 1
fi

echo ""
echo -e "${YELLOW}Step 5: Test HTMX request${NC}"
RESPONSE=$(curl -s -H "HX-Request: true" http://localhost:8080/dashboard)

if echo "$RESPONSE" | grep -q '<div id="main-content">'; then
    echo -e "${GREEN}✓ HTMX requests return content with wrapper${NC}"
else
    echo -e "${RED}✗ HTMX response missing main-content wrapper!${NC}"
    echo ""
    echo "This means server is running OLD code."
    echo ""
    echo "Solution:"
    echo "  1. Stop the server (Ctrl+C or docker-compose down)"
    echo "  2. Rebuild: cargo build --bin api"
    echo "  3. Start: cargo run --bin api"
    echo ""
    exit 1
fi

echo ""
echo -e "${GREEN}=========================================${NC}"
echo -e "${GREEN}All checks passed!${NC}"
echo -e "${GREEN}=========================================${NC}"
echo ""
echo "If navigation still doesn't work:"
echo "1. Open browser DevTools (F12)"
echo "2. Go to Network tab"
echo "3. Click on a job name"
echo "4. Check the response for /dashboard/jobs/{id}"
echo "5. Verify it contains: <div id=\"main-content\">"
echo ""
echo "If response is missing wrapper:"
echo "  → Server needs restart with new code"
echo ""
echo "If response has wrapper but menu still broken:"
echo "  → Check Console tab for JavaScript errors"
echo "  → Check if #main-content element exists after load"
echo ""
