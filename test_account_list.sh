#!/bin/bash

# Start the server in the background
echo "Starting server..."
cd /home/kittyn/maowbot/maowbot-server
cargo run &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to initialize..."
sleep 10

# Run the account list command
echo "Testing account list..."
cd /home/kittyn/maowbot
./target/debug/tui-grpc --server-address localhost:50051 --disable-tui account list

# Kill the server
echo "Stopping server..."
kill $SERVER_PID

echo "Test complete!"