pub fn system_help() -> &'static str {
    r#"System Command
==============

Manage MaowBot server and overlay processes.

Usage:
  system [process] [command]

Processes:
  server    - The MaowBot gRPC server
  overlay   - The VR overlay application

Commands:
  start     - Start the process
  stop      - Stop the process
  status    - Check if the process is running

Examples:
  system server status      # Check if server is running
  system overlay start      # Start the overlay
  system overlay stop       # Stop the overlay
  system server            # Show server status (shorthand)

Note: The TUI automatically starts the server if it's not running when you launch it.
"#
}