// TTV (Twitch) command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::twitch::TwitchCommands};
use crate::tui_module_simple::SimpleTuiModule;
use std::sync::Arc;

/// Helper to require an active Twitch-IRC account name from the TUI state.
fn require_active_account(opt: &Option<String>) -> Result<&str, String> {
    match opt.as_deref() {
        Some(a) => Ok(a),
        None => Err(
            "No active Twitch-IRC account is set. Use 'ttv active <account>' first.".to_string()
        ),
    }
}

/// The main 'ttv' command handler using gRPC
pub async fn handle_ttv_command(
    args: &[&str],
    client: &GrpcClient,
    tui_module: &Arc<SimpleTuiModule>,
) -> String {
    if args.is_empty() {
        return r#"Usage:
  ttv active <accountName>
  ttv join <channelName>
  ttv part <channelName>
  ttv msg <channelName> <message text>
  ttv chat
"#.to_string();
    }

    match args[0].to_lowercase().as_str() {
        "active" => {
            if args.len() < 2 {
                return "Usage: ttv active <accountName>".to_string();
            }
            set_active_account(args[1], tui_module).await
        }
        "join" => {
            if args.len() < 2 {
                return "Usage: ttv join <channelName>".to_string();
            }
            do_join_channel(args[1], client, tui_module).await
        }
        "part" => {
            if args.len() < 2 {
                return "Usage: ttv part <channelName>".to_string();
            }
            do_part_channel(args[1], client, tui_module).await
        }
        "msg" => {
            if args.len() < 3 {
                return "Usage: ttv msg <channelName> <message text...>".to_string();
            }
            let channel = args[1];
            let text = args[2..].join(" ");
            do_send_message(channel, &text, client, tui_module).await
        }
        "chat" => {
            // Enter chat mode in this TUI.
            let mut st = tui_module.ttv_state.lock().unwrap();
            if st.joined_channels.is_empty() {
                return "No channels joined. Use 'ttv join <channelName>' first.".to_string();
            } else {
                st.is_in_chat_mode = true;
                st.current_channel_index = 0;
                return format!(
                    "Chat mode enabled. Type '/quit' to exit. Current channel: {}",
                    st.joined_channels[0]
                );
            }
        }
        _ => "Unrecognized ttv subcommand. Type `ttv` for usage.".to_string(),
    }
}

async fn set_active_account(
    account: &str,
    tui_module: &Arc<SimpleTuiModule>,
) -> String {
    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.active_account = Some(account.to_string());
    }
    
    // Note: We can't save to bot_config via gRPC yet (would need ConfigService)
    // For now just update local state
    format!("Active Twitch-IRC account set to '{}'", account)
}

async fn do_join_channel(
    channel: &str,
    client: &GrpcClient,
    tui_module: &Arc<SimpleTuiModule>,
) -> String {
    let account = {
        let st = tui_module.ttv_state.lock().unwrap();
        match require_active_account(&st.active_account) {
            Ok(a) => a.to_string(),
            Err(e) => return e,
        }
    };

    // Ensure channel has # prefix
    let channel_name = if channel.starts_with('#') {
        channel.to_string()
    } else {
        format!("#{}", channel)
    };

    match TwitchCommands::join_channel(client, &account, &channel_name).await {
        Ok(_) => {
            // Update local joined channels list
            {
                let mut st = tui_module.ttv_state.lock().unwrap();
                if !st.joined_channels.iter().any(|c| c.eq_ignore_ascii_case(&channel_name)) {
                    st.joined_channels.push(channel_name.clone());
                }
            }
            format!("Joined channel '{}'", channel_name)
        }
        Err(e) => format!("Failed to join channel: {}", e),
    }
}

async fn do_part_channel(
    channel: &str,
    client: &GrpcClient,
    tui_module: &Arc<SimpleTuiModule>,
) -> String {
    let account = {
        let st = tui_module.ttv_state.lock().unwrap();
        match require_active_account(&st.active_account) {
            Ok(a) => a.to_string(),
            Err(e) => return e,
        }
    };

    // Ensure channel has # prefix
    let channel_name = if channel.starts_with('#') {
        channel.to_string()
    } else {
        format!("#{}", channel)
    };

    match TwitchCommands::part_channel(client, &account, &channel_name).await {
        Ok(_) => {
            // Update local joined channels list
            {
                let mut st = tui_module.ttv_state.lock().unwrap();
                st.joined_channels.retain(|c| !c.eq_ignore_ascii_case(&channel_name));
                
                // Reset chat mode if no channels left
                if st.joined_channels.is_empty() {
                    st.is_in_chat_mode = false;
                } else if st.current_channel_index >= st.joined_channels.len() {
                    st.current_channel_index = 0;
                }
            }
            format!("Left channel '{}'", channel_name)
        }
        Err(e) => format!("Failed to part channel: {}", e),
    }
}

async fn do_send_message(
    channel: &str,
    text: &str,
    client: &GrpcClient,
    tui_module: &Arc<SimpleTuiModule>,
) -> String {
    let account = {
        let st = tui_module.ttv_state.lock().unwrap();
        match require_active_account(&st.active_account) {
            Ok(a) => a.to_string(),
            Err(e) => return e,
        }
    };

    // Ensure channel has # prefix
    let channel_name = if channel.starts_with('#') {
        channel.to_string()
    } else {
        format!("#{}", channel)
    };

    match TwitchCommands::send_message(client, &account, &channel_name, text).await {
        Ok(result) => {
            format!("Message sent to {} (id: {})", channel_name, result.data.message_id)
        }
        Err(e) => format!("Failed to send message: {}", e),
    }
}