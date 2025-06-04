// Diagnostics command adapter for TUI - system health, logs, and metrics
use maowbot_common_ui::GrpcClient;
use maowbot_proto::maowbot::services::{
    GetSystemStatusRequest, GetCredentialHealthRequest,
    ListActiveRuntimesRequest, ListPluginsRequest,
};

pub async fn handle_diagnostics_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: diagnostics <health|status|metrics|logs|test> [options]".to_string();
    }

    match args[0] {
        "health" => {
            get_system_health(client).await
        }
        
        "status" => {
            get_detailed_status(client).await
        }
        
        "metrics" => {
            get_system_metrics(client).await
        }
        
        "logs" => {
            if args.len() < 2 {
                return "Usage: diagnostics logs <tail|search|level> [options]".to_string();
            }
            handle_logs_command(&args[1..]).await
        }
        
        "test" => {
            run_connectivity_tests(client).await
        }
        
        _ => format!("Unknown diagnostics subcommand: {}", args[0]),
    }
}

async fn get_system_health(client: &GrpcClient) -> String {
    let mut output = String::new();
    output.push_str("=== System Health Check ===\n\n");
    
    // Check plugin status
    let plugin_request = GetSystemStatusRequest {
        include_metrics: true,
    };
    let mut plugin_client = client.plugin.clone();
    
    match plugin_client.get_system_status(plugin_request).await {
        Ok(response) => {
            let status = response.into_inner();
            output.push_str(&format!("Plugin System: ✓ HEALTHY\n"));
            output.push_str(&format!("  Total Plugins: {}\n", status.total_plugins));
            output.push_str(&format!("  Active Plugins: {}\n", status.active_plugins));
            output.push_str(&format!("  System Uptime: {}s\n", status.uptime_seconds));
        }
        Err(e) => {
            output.push_str(&format!("Plugin System: ✗ ERROR - {}\n", e));
        }
    }
    
    output.push_str("\n");
    
    // Check credential health
    let cred_request = GetCredentialHealthRequest {
        platforms: vec![],
    };
    let mut cred_client = client.credential.clone();
    
    match cred_client.get_credential_health(cred_request).await {
        Ok(response) => {
            let health = response.into_inner();
            if let Some(overall) = health.overall {
                output.push_str(&format!("Credential Health: {} {:.1}%\n", 
                    if overall.health_score > 0.8 { "✓" } else { "⚠" },
                    overall.health_score * 100.0
                ));
                output.push_str(&format!("  Healthy Platforms: {}/{}\n", 
                    overall.healthy_platforms, overall.total_platforms
                ));
                output.push_str(&format!("  Total Credentials: {}\n", overall.total_credentials));
                
                // Show any platforms with issues
                for platform_health in health.platform_health {
                    if platform_health.expired_credentials > 0 || platform_health.expiring_soon > 0 {
                        let platform_name = format_platform(platform_health.platform);
                        output.push_str(&format!("  ⚠ {} - {} expired, {} expiring soon\n",
                            platform_name,
                            platform_health.expired_credentials,
                            platform_health.expiring_soon
                        ));
                    }
                }
            }
        }
        Err(e) => {
            output.push_str(&format!("Credential Health: ✗ ERROR - {}\n", e));
        }
    }
    
    output.push_str("\n");
    
    // Check runtime status
    let runtime_request = ListActiveRuntimesRequest {
        platforms: vec![],
    };
    let mut platform_client = client.platform.clone();
    
    match platform_client.list_active_runtimes(runtime_request).await {
        Ok(response) => {
            let runtimes = response.into_inner().runtimes;
            output.push_str(&format!("Active Runtimes: {}\n", runtimes.len()));
            
            for runtime in runtimes {
                output.push_str(&format!("  ✓ {} - {} ({}s uptime)\n",
                    runtime.platform,
                    runtime.account_name,
                    runtime.uptime_seconds
                ));
            }
        }
        Err(e) => {
            output.push_str(&format!("Runtime Status: ✗ ERROR - {}\n", e));
        }
    }
    
    output
}

async fn get_detailed_status(client: &GrpcClient) -> String {
    let mut output = String::new();
    output.push_str("=== Detailed System Status ===\n\n");
    
    // Get plugin details
    let request = ListPluginsRequest {
        active_only: false,
        include_system_plugins: true,
    };
    let mut plugin_client = client.plugin.clone();
    
    match plugin_client.list_plugins(request).await {
        Ok(response) => {
            let plugins = response.into_inner().plugins;
            output.push_str(&format!("Plugins ({} total):\n", plugins.len()));
            
            for plugin in plugins {
                let plugin_data = &plugin.plugin.as_ref().unwrap();
                let status_icon = if plugin_data.is_connected {
                    "✓"
                } else {
                    "✗"
                };
                
                output.push_str(&format!("  {} {} - v{}\n",
                    status_icon,
                    plugin_data.plugin_name,
                    plugin_data.version
                ));
                
                if let Some(author) = plugin_data.metadata.get("author") {
                    output.push_str(&format!("      Author: {}\n", author));
                }
            }
        }
        Err(e) => {
            output.push_str(&format!("Error listing plugins: {}\n", e));
        }
    }
    
    output.push_str("\n");
    
    // Get platform runtime details
    let runtime_request = ListActiveRuntimesRequest {
        platforms: vec![],
    };
    let mut platform_client = client.platform.clone();
    
    match platform_client.list_active_runtimes(runtime_request).await {
        Ok(response) => {
            let runtimes = response.into_inner().runtimes;
            output.push_str(&format!("Platform Runtimes ({} active):\n", runtimes.len()));
            
            for runtime in runtimes {
                output.push_str(&format!("  {} - {}\n", runtime.platform, runtime.account_name));
                output.push_str(&format!("    Runtime ID: {}\n", runtime.runtime_id));
                output.push_str(&format!("    Uptime: {}s\n", runtime.uptime_seconds));
                
                if let Some(stats) = runtime.stats {
                    output.push_str(&format!("    Messages Sent: {}\n", stats.messages_sent));
                    output.push_str(&format!("    Messages Received: {}\n", stats.messages_received));
                    output.push_str(&format!("    Errors: {}\n", stats.errors_count));
                }
            }
        }
        Err(e) => {
            output.push_str(&format!("Error getting runtime details: {}\n", e));
        }
    }
    
    output
}

async fn get_system_metrics(_client: &GrpcClient) -> String {
    // This would require a metrics service in the proto files
    // For now, return a placeholder
    let mut output = String::new();
    output.push_str("=== System Metrics ===\n\n");
    output.push_str("Metrics collection not yet implemented in gRPC services.\n");
    output.push_str("\nSuggested metrics to track:\n");
    output.push_str("  - Message throughput (msgs/sec)\n");
    output.push_str("  - Command processing time (avg/p95/p99)\n");
    output.push_str("  - Memory usage by component\n");
    output.push_str("  - Database query performance\n");
    output.push_str("  - API response times\n");
    output.push_str("  - Error rates by platform\n");
    
    output
}

async fn handle_logs_command(args: &[&str]) -> String {
    match args[0] {
        "tail" => {
            let lines = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(50);
            format!("Log tailing not yet implemented. Would show last {} lines.", lines)
        }
        
        "search" => {
            if args.len() < 2 {
                return "Usage: diagnostics logs search <pattern>".to_string();
            }
            let pattern = args[1];
            format!("Log search not yet implemented. Would search for '{}'.", pattern)
        }
        
        "level" => {
            if args.len() < 2 {
                return "Usage: diagnostics logs level <debug|info|warn|error>".to_string();
            }
            let level = args[1];
            format!("Log level filtering not yet implemented. Would filter by '{}'.", level)
        }
        
        _ => "Usage: diagnostics logs <tail|search|level> [options]".to_string(),
    }
}

async fn run_connectivity_tests(client: &GrpcClient) -> String {
    let mut output = String::new();
    output.push_str("=== Connectivity Tests ===\n\n");
    
    // Test gRPC connection
    output.push_str("gRPC Connection: ");
    let test_request = GetSystemStatusRequest {
        include_metrics: false,
    };
    let mut plugin_client = client.plugin.clone();
    
    match plugin_client.get_system_status(test_request).await {
        Ok(_) => output.push_str("✓ PASS\n"),
        Err(e) => output.push_str(&format!("✗ FAIL - {}\n", e)),
    }
    
    // Test database connection (via any query)
    output.push_str("Database Connection: ");
    let list_request = maowbot_proto::maowbot::services::ListUsersRequest {
        filter: None,
        page: Some(maowbot_proto::maowbot::common::PageRequest {
            page_size: 1,
            page_token: String::new(),
        }),
        order_by: String::new(),
        descending: false,
    };
    let mut user_client = client.user.clone();
    
    match user_client.list_users(list_request).await {
        Ok(_) => output.push_str("✓ PASS\n"),
        Err(e) => output.push_str(&format!("✗ FAIL - {}\n", e)),
    }
    
    // Test each platform's connectivity
    output.push_str("\nPlatform Connectivity:\n");
    let platforms = vec!["twitch-irc", "twitch-eventsub", "discord", "vrchat"];
    
    for platform in platforms {
        output.push_str(&format!("  {}: ", platform));
        // This would require actually testing platform connections
        output.push_str("⚠ Not implemented\n");
    }
    
    output
}

fn format_platform(platform: i32) -> &'static str {
    match maowbot_proto::maowbot::common::Platform::try_from(platform) {
        Ok(maowbot_proto::maowbot::common::Platform::TwitchHelix) => "Twitch",
        Ok(maowbot_proto::maowbot::common::Platform::TwitchIrc) => "Twitch-IRC",
        Ok(maowbot_proto::maowbot::common::Platform::TwitchEventsub) => "Twitch-EventSub",
        Ok(maowbot_proto::maowbot::common::Platform::Discord) => "Discord",
        Ok(maowbot_proto::maowbot::common::Platform::Vrchat) => "VRChat",
        _ => "Unknown",
    }
}