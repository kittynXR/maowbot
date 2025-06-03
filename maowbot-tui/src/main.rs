// Standalone TUI client using gRPC
use maowbot_common_ui::GrpcClient;
use maowbot_tui::{commands::dispatch_grpc, SimpleTuiModule};
use tokio::io::{AsyncBufReadExt, BufReader};
use std::sync::Arc;
use std::io::{stdout, Write};

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

    // Create a minimal TUI module for the gRPC client
    let tui_module = Arc::new(SimpleTuiModule::new());

    println!("\nType 'help' for available commands.\n");

    // Main input loop
    let mut reader = BufReader::new(tokio::io::stdin()).lines();
    
    loop {
        print!("{}", tui_module.prompt_string());
        stdout().flush()?;

        let line = match reader.next_line().await? {
            Some(line) => line.trim().to_string(),
            None => break, // EOF
        };

        if line.is_empty() {
            continue;
        }

        // Check if we're in special chat modes
        {
            let is_in_ttv_chat = tui_module.ttv_state.lock().unwrap().is_in_chat_mode;
            if is_in_ttv_chat {
                if tui_module.handle_ttv_chat_line(&line, &client).await {
                    continue;
                }
            }
        }

        {
            let is_in_osc_chat = tui_module.osc_state.lock().unwrap().is_in_chat_mode;
            if is_in_osc_chat {
                if tui_module.handle_osc_chat_line(&line, &client).await {
                    continue;
                }
            }
        }

        // Otherwise, interpret line as a command
        let (quit_requested, output) = dispatch_grpc(&line, &client, &tui_module).await;
        
        if let Some(msg) = output {
            println!("{}", msg);
        }
        
        if quit_requested {
            break;
        }
    }

    println!("Goodbye!");
    Ok(())
}