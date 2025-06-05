#!/bin/bash

# Test script for AI service functionality

echo "Testing AI Service Configuration..."
echo "=================================="

# Set environment variable for testing
export OPENAI_API_KEY="test-key-123"

# Start the server in the background
echo "Starting server with test API key..."
cd maowbot-server && cargo run -- --nuke-database-and-start-fresh > ../server_log.txt 2>&1 &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to initialize..."
sleep 10

# Check if server started successfully
if ps -p $SERVER_PID > /dev/null; then
    echo "Server started successfully (PID: $SERVER_PID)"
    
    # Extract relevant logs
    echo -e "\nAI Service Initialization Logs:"
    echo "-------------------------------"
    grep -E "AI SERVICE:|AI_API_IMPL:|AiApiImpl" ../server_log.txt | tail -20
    
    # Check if AI service was configured
    echo -e "\nChecking AI service configuration:"
    echo "----------------------------------"
    if grep -q "Configured OpenAI provider from environment variable" ../server_log.txt; then
        echo "✓ AI service configured from environment variable"
    else
        echo "✗ AI service NOT configured from environment variable"
    fi
    
    if grep -q "AI service enabled flag set to true" ../server_log.txt; then
        echo "✓ AI service enabled flag is true"
    else
        echo "✗ AI service enabled flag is NOT true"
    fi
    
    # Kill the server
    echo -e "\nStopping server..."
    kill $SERVER_PID
    wait $SERVER_PID 2>/dev/null
else
    echo "Server failed to start. Check server_log.txt for details."
    tail -50 ../server_log.txt
fi

# Cleanup
rm -f ../server_log.txt

echo -e "\nTest complete."