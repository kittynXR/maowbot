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
//!   osc raw
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
  osc raw
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
        "raw" => {
            // Start the OSC raw packet listener
            println!("Starting OSC raw packet monitor. Press Ctrl+C to stop.");

            match bot_api.osc_take_raw_receiver().await {
                Ok(Some(mut rx)) => {
                    // Store the handle so it can be aborted during shutdown
                    let task_handle = tokio::spawn(async move {
                        println!("Raw OSC packet monitoring active.");
                        println!("Waiting for incoming OSC packets...");

                        while let Some(packet) = rx.recv().await {
                            match packet {
                                rosc::OscPacket::Message(msg) => {
                                    println!("OSC Message: addr={}, args={:?}", msg.addr, msg.args);
                                }
                                rosc::OscPacket::Bundle(bundle) => {
                                    println!("OSC Bundle: time={:?}, {} messages",
                                             bundle.timetag, bundle.content.len());
                                    for (i, content) in bundle.content.iter().enumerate() {
                                        println!("  [{}]: {:?}", i, content);
                                    }
                                }
                            }
                        }

                        println!("OSC raw packet monitor stopped.");
                    });

                    // Store the task handle in a global registry or similar
                    // For now we let it run but in a proper implementation we'd
                    // want to keep track of this to abort it during shutdown

                    "OSC raw packet monitor started. Messages will appear in console.".to_string()
                },
                Ok(None) => "No OSC receiver available. Try starting OSC first.".to_string(),
                Err(e) => format!("Error getting OSC receiver: {:?}", e),
            }
        },
        _ => "Unknown subcommand. Type 'osc' for help.".to_string(),
    }
}