#!/bin/bash

# Test script for server shutdown command

echo "Building the project..."
cargo build -p maowbot-server -p maowbot-tui

echo "Starting server in background..."
cargo run -p maowbot-server &
SERVER_PID=$!

echo "Waiting for server to start..."
sleep 5

echo "Running TUI command to shutdown server..."
echo "system shutdown testing 10" | cargo run -p maowbot-tui --bin tui-grpc

echo "Waiting for shutdown..."
sleep 15

# Check if server is still running
if ps -p $SERVER_PID > /dev/null; then
    echo "Server still running, killing it..."
    kill $SERVER_PID
else
    echo "Server shut down successfully!"
fi