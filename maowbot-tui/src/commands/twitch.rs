use std::sync::Arc;
use maowbot_common::traits::api::{BotApi, BotConfigApi, TwitchApi};
use crate::tui_module::TuiModule;

/// Helper to require an active Twitch-IRC account name from the TUI state.
fn require_active_account(opt: &Option<String>) -> Result<&str, String> {
    match opt.as_deref() {
        Some(a) => Ok(a),
        None => Err(
            "No active Twitch-IRC account is set. Use 'twitch active <account>' first.".to_string()
        ),
    }
}

/// The main 'twitch' command handler.
///
/// Old subcommands like `twitch broadcaster <channel>` or `twitch secondary <account>` have been removed.
/// The broadcaster is now inferred from any credential with `is_broadcaster = true`.
pub async fn handle_twitch_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
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
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.active_account = Some(account.to_string());
    }
    // Optionally store in bot_config, if needed
    if let Err(e) = bot_api
        .set_bot_config_value("ttv_active_account", account)
        .await
    {
        return format!("Error storing ttv_active_account => {:?}", e);
    }
    format!("Active Twitch-IRC account set to '{}'", account)
}

async fn do_join_channel(
    channel: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = strip_channel_prefix(channel);

    let (already_joined, maybe_acct) = {
        let mut st = tui_module.ttv_state.lock().unwrap();
        let already = st.joined_channels
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&chname));
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

    // Ensure the platform runtime is started:
    if let Err(e) = bot_api.start_platform_runtime("twitch-irc", active_account).await {
        return format!("Error starting twitch-irc => {:?}", e);
    }

    // Actually join the IRC channel
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
        let pos = st.joined_channels
            .iter()
            .position(|c| c.eq_ignore_ascii_case(&chname));
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

/// Utility to remove any leading '#' from the channel name.
fn strip_channel_prefix(raw: &str) -> String {
    raw.trim().trim_start_matches('#').to_string()
}
