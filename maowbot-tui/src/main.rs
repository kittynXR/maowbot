// Simple main for testing gRPC commands
use maowbot_common_ui::GrpcClient;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("MaowBot TUI (gRPC mode)");
    println!("Connecting to gRPC server at https://127.0.0.1:9999...");

    // Connect to gRPC server
    let client = match GrpcClient::connect("https://127.0.0.1:9999").await {
        Ok(c) => {
            println!("✅ Connected to gRPC server!");
            c
        }
        Err(e) => {
            println!("❌ Failed to connect to gRPC server: {}", e);
            println!("Make sure maowbot-server is running!");
            return Err(e.into());
        }
    };

    println!("\nAvailable commands:");
    println!("  user <add|remove|edit|info|search|list> - User management");
    println!("  platform <add|remove|list|show> - Platform configuration"); 
    println!("  ttv <msg|join|part|info|follow|stream|ban|unban> - Twitch commands");
    println!("  discord <liverole|guilds|channels|send|member|members> - Discord commands");
    println!("  command <list|enable|disable|setcooldown|setwarnonce> - Command management");
    println!("  redeem <list|info|add|enable|pause|setcost|sync> - Redeem management");
    println!("  test_grpc - Test gRPC connectivity");
    println!("  quit - Exit");
    println!("\nType 'help' for more information.\n");

    // Main input loop
    let mut reader = BufReader::new(tokio::io::stdin()).lines();
    
    loop {
        print!("tui> ");
        use std::io::{stdout, Write};
        stdout().flush()?;

        let line = match reader.next_line().await? {
            Some(line) => line.trim().to_string(),
            None => break, // EOF
        };

        if line.is_empty() {
            continue;
        }

        // Simple command dispatcher
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let output = match parts[0] {
            "user" => {
                use maowbot_tui::commands::user_adapter;
                user_adapter::handle_user_command(&parts[1..], &client).await
            }
            "platform" => {
                use maowbot_tui::commands::platform_adapter;
                platform_adapter::handle_platform_command(&parts[1..], &client).await
            }
            "ttv" => {
                use maowbot_tui::commands::ttv_simple_adapter;
                ttv_simple_adapter::handle_ttv_command(&parts[1..], &client).await
            }
            "discord" => {
                use maowbot_tui::commands::discord_adapter;
                discord_adapter::handle_discord_command(&parts[1..], &client).await
            }
            "command" => {
                use maowbot_tui::commands::command_adapter;
                command_adapter::handle_command_command(&parts[1..], &client).await
            }
            "redeem" => {
                use maowbot_tui::commands::redeem_adapter;
                redeem_adapter::handle_redeem_command(&parts[1..], &client).await
            }
            "test_grpc" => {
                use maowbot_tui::commands::test_grpc;
                test_grpc::handle_test_grpc_command(&parts[1..]).await
            }
            "quit" => break,
            "help" => {
                "Available commands:\n  \
                user <add|remove|edit|info|search|list>\n  \
                platform <add|remove|list|show>\n  \
                ttv <msg|join|part|info|follow|stream|ban|unban>\n  \
                discord <liverole|guilds|channels|send|member|members>\n  \
                command <list|enable|disable|setcooldown|setwarnonce>\n  \
                redeem <list|info|add|enable|pause|setcost|sync>\n  \
                test_grpc\n  \
                quit".to_string()
            }
            _ => format!("Unknown command '{}'. Type 'help' for usage.", parts[0]),
        };

        println!("{}", output);
    }

    println!("Goodbye!");
    Ok(())
}