//! maowbot-tui/src/commands/osc.rs
//!
//! Handles "osc" commands in the TUI, including:
//!   osc start
//!   osc stop
//!   osc restart
//!   osc chatbox <message>
//!     (if no <message>, open a REPL-like chat loop until /quit)
//!   osc status
//!   osc discover
//!

use std::sync::Arc;
use maowbot_common::traits::api::BotApi;
use crate::tui_module::TuiModule;

/// Dispatches the "osc" subcommands.
pub async fn handle_osc_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return r#"Usage:
  osc start
  osc stop
  osc restart
  osc chatbox [message...]
  osc status
  osc discover
"#.to_string();
    }
    match args[0] {
        "start" => {
            match bot_api.osc_start().await {
                Ok(_) => "OSC started.".to_string(),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "stop" => {
            match bot_api.osc_stop().await {
                Ok(_) => "OSC stopped.".to_string(),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "restart" => {
            match bot_api.osc_restart().await {
                Ok(_) => "OSC restarted.".to_string(),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "chatbox" => {
            // If there's more than 1 sub-arg, treat it as a single message
            if args.len() > 1 {
                let message = args[1..].join(" ");
                match bot_api.osc_chatbox(&message).await {
                    Ok(_) => format!("(sent) {}", message),
                    Err(e) => format!("Error => {:?}", e),
                }
            } else {
                // No argument => go into chatbox REPL mode
                let mut st = tui_module.osc_state.lock().unwrap();
                st.is_in_chat_mode = true;
                drop(st);
                "Entering OSC chatbox mode. Type /quit to exit.".to_string()
            }
        }
        "status" => {
            match bot_api.osc_status().await {
                Ok(stat) => {
                    format!(
                        "OSC running={} port={:?}, OSCQuery={} http_port={:?}",
                        stat.is_running,
                        stat.listening_port,
                        stat.is_oscquery_running,
                        stat.oscquery_port
                    )
                }
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "discover" => {
            match bot_api.osc_discover_peers().await {
                Ok(list) if list.is_empty() => "No local OSCQuery services discovered.".to_string(),
                Ok(list) => {
                    let lines = list.into_iter()
                        .map(|name| format!(" - {name}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("Discovered:\n{}", lines)
                }
                Err(e) => format!("Error => {:?}", e),
            }
        }
        _ => "Unknown subcommand. Type 'osc' for help.".to_string(),
    }
}
