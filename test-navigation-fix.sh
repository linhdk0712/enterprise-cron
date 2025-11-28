#!/bin/bash

echo "=== Testing Navigation Fix ==="
echo ""
echo "1. Restart API server to load new templates"
echo "   Command: cargo run --bin api"
echo ""
echo "2. Test sequence:"
echo "   a. Open browser: http://localhost:8080/dashboard"
echo "   b. Click 'Jobs' menu"
echo "   c. Click on a job name to view details"
echo "   d. Try clicking 'Dashboard' or 'Executions' menu"
echo ""
echo "Expected result: Menu should work (navigate to other pages)"
echo "Previous bug: Menu not clickable after viewing job details"
echo ""
echo "3. Check browser console for:"
echo "   - No HTMX errors"
echo "   - Successful requests to /dashboard/jobs/{id}"
echo "   - Response should contain '<div id=\"main-content\">'"
echo ""
echo "4. Verify with curl:"
echo ""

# Test HTMX request (should return content with wrapper)
echo "Testing HTMX request (should have main-content wrapper):"
curl -s -H "HX-Request: true" http://localhost:8080/dashboard/jobs/bbd0f989-7c13-4c19-b8a6-b258a1abb4da | grep -o '<div id="main-content">' | head -1

if [ $? -eq 0 ]; then
    echo "✅ PASS: HTMX request returns content with #main-content wrapper"
else
    echo "❌ FAIL: HTMX request missing #main-content wrapper"
fi

echo ""
echo "Testing direct request (should have layout):"
curl -s http://localhost:8080/dashboard/jobs/bbd0f989-7c13-4c19-b8a6-b258a1abb4da | grep -o '<title>' | head -1

if [ $? -eq 0 ]; then
    echo "✅ PASS: Direct request returns full page with layout"
else
    echo "❌ FAIL: Direct request missing layout"
fi

echo ""
echo "=== Test Complete ==="
