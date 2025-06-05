#!/bin/bash
# Run the standalone MaowBot TUI client

# Check if server is running
if ! pgrep -f "maowbot-server" > /dev/null; then
    echo "⚠️  Warning: maowbot-server doesn't appear to be running!"
    echo "Start the server first with: cargo run --bin maowbot-server"
    echo ""
fi

# Ensure certificates are available
if [ ! -f "certs/server.crt" ]; then
    echo "📋 Copying server certificate..."
    mkdir -p certs
    if [ -f "target/debug/certs/server.crt" ]; then
        cp target/debug/certs/server.crt certs/
    else
        echo "⚠️  Warning: No server certificate found. TLS connection may fail."
    fi
fi

echo "🚀 Starting MaowBot TUI (gRPC client)..."
echo ""

# Run with quiet output to suppress warnings
RUSTFLAGS="-A warnings" cargo run --quiet --bin tui-grpc