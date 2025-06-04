// Standalone TUI client using gRPC
use maowbot_common_ui::{GrpcClient, ProcessManager};
use maowbot_tui::{commands::dispatch_grpc, SimpleTuiModule, completion::TuiCompleter};
use std::sync::Arc;
use rustyline::error::ReadlineError;
use rustyline::Editor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("MaowBot TUI (gRPC mode)");
    
    // Create process manager
    let process_manager = Arc::new(ProcessManager::new());
    
    // Ensure server is running
    println!("Checking server status...");
    let server_url = process_manager.ensure_server_running().await?;
    
    println!("Connecting to gRPC server at {}...", server_url);

    // Connect to gRPC server
    let client = match GrpcClient::connect(&server_url).await {
        Ok(c) => {
            println!("✅ Connected to gRPC server!");
            c
        }
        Err(e) => {
            println!("❌ Failed to connect to gRPC server: {}", e);
            return Err(e.into());
        }
    };

    // Create a minimal TUI module for the gRPC client
    let tui_module = Arc::new(SimpleTuiModule::new());

    println!("\nType 'help' for available commands.\n");

    // Initialize readline with tab completion
    let mut rl = Editor::<TuiCompleter, _>::new()?;
    rl.set_helper(Some(TuiCompleter::new()));
    
    // Load history if it exists
    let history_path = dirs::home_dir()
        .map(|mut path| {
            path.push(".maowbot_tui_history");
            path
        });
    
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }
    
    // Main input loop
    loop {
        let prompt = tui_module.prompt_string();
        
        let line = match rl.readline(&prompt) {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                line.trim().to_string()
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
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
        let (quit_requested, output) = dispatch_grpc(&line, &client, &tui_module, &process_manager).await;
        
        if let Some(msg) = output {
            println!("{}", msg);
        }
        
        if quit_requested {
            break;
        }
    }

    // Save history
    if let Some(path) = history_path {
        let _ = rl.save_history(&path);
    }
    
    println!("Goodbye!");
    
    // Stop server if we started it (optional - could make this configurable)
    // For now, we'll leave the server running so other clients can connect
    
    Ok(())
}