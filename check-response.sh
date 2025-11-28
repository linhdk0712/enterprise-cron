#!/bin/bash

echo "Checking job details response..."
echo ""

# Get a job ID from jobs list
JOB_ID=$(curl -s http://localhost:8080/api/jobs | jq -r '.[0].id' 2>/dev/null)

if [ -z "$JOB_ID" ] || [ "$JOB_ID" = "null" ]; then
    echo "Using hardcoded job ID..."
    JOB_ID="bbd0f989-7c13-4c19-b8a6-b258a1abb4da"
fi

echo "Testing job ID: $JOB_ID"
echo ""

# Test HTMX request
echo "=== HTMX Request (with HX-Request header) ==="
RESPONSE=$(curl -s -H "HX-Request: true" "http://localhost:8080/dashboard/jobs/$JOB_ID")

# Check for main-content
if echo "$RESPONSE" | grep -q '<div id="main-content">'; then
    echo "✅ Response contains <div id=\"main-content\">"
    
    # Count how many times it appears
    COUNT=$(echo "$RESPONSE" | grep -o '<div id="main-content">' | wc -l)
    echo "   Found $COUNT occurrence(s)"
    
    # Show first 50 lines
    echo ""
    echo "First 50 lines of response:"
    echo "$RESPONSE" | head -50
else
    echo "❌ Response MISSING <div id=\"main-content\">"
    echo ""
    echo "Response preview (first 100 lines):"
    echo "$RESPONSE" | head -100
    echo ""
    echo "This means server is still running OLD code!"
fi

echo ""
echo "=== Direct Request (without HX-Request header) ==="
RESPONSE2=$(curl -s "http://localhost:8080/dashboard/jobs/$JOB_ID")

if echo "$RESPONSE2" | grep -q '<title>'; then
    echo "✅ Direct request returns full page with layout"
else
    echo "❌ Direct request missing layout"
fi

# Check if response has main-content in body
if echo "$RESPONSE2" | grep -q '<div id="main-content">'; then
    echo "✅ Direct request also has main-content wrapper"
else
    echo "⚠️  Direct request missing main-content (this is OK if using layout)"
fi
