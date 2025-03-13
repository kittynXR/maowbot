use std::sync::Arc;
use maowbot_common::traits::api::{BotApi, BotConfigApi, TwitchApi};
use crate::tui_module::TuiModule;

/// Helper to require that the active_account is `Some(...)`.
/// Returns &str if present, or an error String if None.
fn require_active_account(opt: &Option<String>) -> Result<&str, String> {
    match opt.as_deref() {
        Some(a) => Ok(a),
        None => Err("No active Twitch-IRC account is set. Use 'ttv active <account>' first.".to_string()),
    }
}

pub async fn handle_ttv_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return r#"Usage:
  ttv broadcaster <channelName>
  ttv secondary <accountName>
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
            set_active_account(args[1], bot_api, tui_module).await
        }

        "join" => {
            if args.len() < 2 {
                return "Usage: ttv join <channelName>".to_string();
            }
            do_join_channel(args[1], bot_api, tui_module).await
        }

        "part" => {
            if args.len() < 2 {
                return "Usage: ttv part <channelName>".to_string();
            }
            do_part_channel(args[1], bot_api, tui_module).await
        }

        "msg" => {
            if args.len() < 3 {
                return "Usage: ttv msg <channelName> <message text...>".to_string();
            }
            let channel = args[1];
            let text = args[2..].join(" ");
            do_send_message(channel, &text, bot_api, tui_module).await
        }

        "chat" => {
            // Enter chat mode
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

        "broadcaster" => {
            if args.len() < 2 {
                return "Usage: ttv broadcaster <channelName>".to_string();
            }
            set_named_broadcaster(args[1], bot_api, tui_module).await
        }

        "secondary" => {
            if args.len() < 2 {
                return "Usage: ttv secondary <accountName>".to_string();
            }
            set_secondary_account(args[1], bot_api, tui_module).await
        }

        _ => "Unrecognized ttv subcommand. Type `ttv` for usage.".to_string(),
    }
}

async fn set_active_account(account: &str, bot_api: &Arc<dyn BotApi>, tui_module: &Arc<TuiModule>) -> String {
    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.active_account = Some(account.to_string());
    }
    // Store in bot_config old-style:
    if let Err(e) = bot_api.set_bot_config_value("ttv_active_account", account).await {
        return format!("Error storing ttv_active_account => {:?}", e);
    }
    format!("Active Twitch account set to '{}'", account)
}

async fn do_join_channel(
    channel: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = strip_channel_prefix(channel);

    let (already_joined, maybe_acct) = {
        let mut st = tui_module.ttv_state.lock().unwrap();
        let already = st.joined_channels.iter().any(|c| c.eq_ignore_ascii_case(&chname));
        if !already {
            st.joined_channels.push(chname.clone());
            st.joined_channels.sort();
        }
        (already, st.active_account.clone())
    };

    if already_joined {
        return format!("We're already joined to channel '{}'.", chname);
    }

    let active_account = match require_active_account(&maybe_acct) {
        Ok(a) => a,
        Err(e) => return e,
    };

    // Ensure runtime is started
    if let Err(e) = bot_api.start_platform_runtime("twitch-irc", active_account).await {
        return format!("Error starting twitch-irc => {:?}", e);
    }
    match bot_api.join_twitch_irc_channel(active_account, &chname).await {
        Ok(_) => format!("Joined channel '{}'. Now receiving messages.", chname),
        Err(e) => format!("Error joining '{}': {:?}", chname, e),
    }
}

async fn do_part_channel(
    channel: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = strip_channel_prefix(channel);

    let (pos_opt, maybe_acct) = {
        let st = tui_module.ttv_state.lock().unwrap();
        let pos = st.joined_channels.iter().position(|c| c.eq_ignore_ascii_case(&chname));
        (pos, st.active_account.clone())
    };

    if pos_opt.is_none() {
        return format!("Not currently joined to '{}'.", chname);
    }

    let active_account = match require_active_account(&maybe_acct) {
        Ok(a) => a,
        Err(e) => return e,
    };

    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        if let Some(idx) = pos_opt {
            st.joined_channels.remove(idx);
        }
    }

    match bot_api.part_twitch_irc_channel(active_account, &chname).await {
        Ok(_) => format!("Parted channel '{}'.", chname),
        Err(e) => format!("Error parting '{}': {:?}", chname, e),
    }
}

async fn do_send_message(
    channel: &str,
    text: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = strip_channel_prefix(channel);

    let maybe_acct = {
        let st = tui_module.ttv_state.lock().unwrap();
        st.active_account.clone()
    };

    let active_acc = match require_active_account(&maybe_acct) {
        Ok(a) => a,
        Err(e) => return e,
    };

    match bot_api.send_twitch_irc_message(active_acc, &chname, text).await {
        Ok(_) => format!("[{}] {}: {}", chname, active_acc, text),
        Err(e) => format!("Error sending msg to '{}': {:?}", chname, e),
    }
}

async fn set_named_broadcaster(
    channel: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = strip_channel_prefix(channel);

    // Store old-style: "ttv_broadcaster_channel"
    if let Err(e) = bot_api.set_bot_config_value("ttv_broadcaster_account", &chname).await {
        return format!("Error storing ttv_broadcaster_channel => {:?}", e);
    }

    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.broadcaster_channel = Some(chname.clone());
    }

    format!("Broadcaster channel set to '{}'. Will auto-join on start.", chname)
}

async fn set_secondary_account(
    account: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    // Store old-style: "ttv_secondary_account"
    if let Err(e) = bot_api.set_bot_config_value("ttv_secondary_account", account).await {
        return format!("Error storing ttv_secondary_account => {:?}", e);
    }

    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.secondary_account = Some(account.to_string());
    }

    format!("Secondary Twitch-IRC account set to '{}'. This will be used to respond by default.", account)
}

fn strip_channel_prefix(raw: &str) -> String {
    raw.trim().trim_start_matches('#').to_string()
}
