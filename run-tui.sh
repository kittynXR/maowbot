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
echo "Options:"
echo "  --stop-server-on-exit    Stop the server when TUI exits"
echo "  --no-autostart          Don't start server automatically"
echo "  --server-url <URL>      Connect to specific server URL"
echo ""

# Run with quiet output to suppress warnings
# Pass through any command line arguments
RUSTFLAGS="-A warnings" cargo run --quiet --bin tui-grpc -- "$@"