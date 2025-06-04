/// Detailed help text for the "diagnostics" command
pub const DIAGNOSTICS_HELP_TEXT: &str = r#"Diagnostics Command:
  System health monitoring, troubleshooting, and performance analysis.

Subcommands:
  diagnostics health
      Performs a comprehensive health check of all bot systems including:
      - Plugin system status
      - Credential health and expiration
      - Active platform runtimes
      - Overall system status

  diagnostics status
      Shows detailed status information for all components:
      - Plugin details with versions and authors
      - Runtime statistics for each platform
      - Connection states and uptime

  diagnostics metrics
      Display system performance metrics (not yet implemented):
      - Message throughput
      - Command processing times
      - Memory usage
      - Error rates

  diagnostics logs tail [lines]
      Show the last N lines of logs (default: 50).
      Note: Not yet implemented in gRPC.

  diagnostics logs search <pattern>
      Search logs for a specific pattern.
      Note: Not yet implemented in gRPC.

  diagnostics logs level <debug|info|warn|error>
      Filter logs by severity level.
      Note: Not yet implemented in gRPC.

  diagnostics test
      Run connectivity tests to verify:
      - gRPC connection
      - Database connection
      - Platform API connectivity

Examples:
  diagnostics health
  diagnostics status
  diagnostics test
  diagnostics logs tail 100
  diagnostics logs search "error"
  diagnostics logs level error

Aliases:
  'diag' can be used as a shorthand for 'diagnostics'
"#;