# Shutdown Implementation

## Overview

The `system shutdown` command provides a graceful way to shut down the server and all UI components, regardless of how the server was started.

## Usage

```bash
# Basic shutdown with default 30-second grace period
system shutdown

# Shutdown with a custom reason
system shutdown "maintenance"

# Shutdown with custom reason and grace period (in seconds)
system shutdown "maintenance" 60
```

## Implementation Details

### Server Side (maowbot-server)

1. The shutdown is handled by the `ConfigService` gRPC service
2. When a shutdown is requested:
   - A grace period timer is started (default 30 seconds)
   - The event bus is signaled to shut down after the grace period
   - A response is sent back with the scheduled shutdown time

3. The server's main loop monitors the event bus shutdown signal
4. When shutdown is detected:
   - gRPC server is stopped
   - PostgreSQL is stopped (if managed)
   - Background tasks are aborted
   - Process exits with code 0

### Client Side (maowbot-tui)

1. The TUI sends the shutdown request via gRPC
2. If the request is accepted:
   - The TUI stops any overlay process it manages
   - The TUI itself exits after displaying the shutdown message

## Key Changes

1. **Server Exit**: Added `std::process::exit(0)` to ensure the server process terminates completely, even when started from console.

2. **UI Coordination**: The TUI now stops the overlay process when shutdown is requested, ensuring clean shutdown of all components.

3. **Auto-quit**: The TUI automatically quits after issuing a successful shutdown command.

## Testing

To test the shutdown functionality:

```bash
# Start server from console
cargo run -p maowbot-server

# In another terminal, issue shutdown
echo "system shutdown" | cargo run -p maowbot-tui --bin maowbot-tui-grpc

# Server should shut down gracefully after the grace period
```

## Notes

- The shutdown is graceful, allowing time for cleanup
- All managed processes (overlay, etc.) are stopped
- The shutdown reason and time are logged
- Works regardless of how the server was started (TUI-managed or standalone)