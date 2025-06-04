pub fn system_help() -> &'static str {
    r#"System Command
==============

Manage MaowBot server and overlay processes.

Usage:
  system [process] [command]
  system shutdown [reason] [grace_period_seconds]

Processes:
  server    - The MaowBot gRPC server
  overlay   - The VR overlay application

Commands:
  start     - Start the process
  stop      - Stop the process
  status    - Check if the process is running
  shutdown  - Request graceful server shutdown (via gRPC)

Examples:
  system server status                    # Check if server is running
  system overlay start                    # Start the overlay
  system overlay stop                     # Stop the overlay
  system server                           # Show server status (shorthand)
  system shutdown                         # Shutdown server with 30s grace period
  system shutdown "maintenance" 60        # Shutdown for maintenance in 60 seconds

Note: 
- The TUI automatically starts the server if it's not running when you launch it.
- The 'shutdown' command requires an active gRPC connection to the server.
- The 'stop' command forcefully terminates the process, while 'shutdown' is graceful.
"#
}