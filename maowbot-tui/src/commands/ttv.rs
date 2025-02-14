// File: maowbot-tui/src/commands/ttv.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;
use crate::tui_module::TuiModule;

/// Dispatches subcommands for `ttv`.
pub async fn handle_ttv_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return r#"Usage:
  ttv active <accountName>
  ttv join <channel>
  ttv part <channel>
  ttv msg <channel> <message text>
  ttv chat
  ttv default <channel>
"#.to_string();
    }

    match args[0].to_lowercase().as_str() {
        "active" => {
            if args.len() < 2 {
                "Usage: ttv active <accountName>".to_string()
            } else {
                set_active_account(args[1], tui_module)
            }
        }
        "join" => {
            if args.len() < 2 {
                "Usage: ttv join <channelName>".to_string()
            } else {
                do_join_channel(args[1], bot_api, tui_module).await
            }
        }
        "part" => {
            if args.len() < 2 {
                "Usage: ttv part <channelName>".to_string()
            } else {
                do_part_channel(args[1], bot_api, tui_module).await
            }
        }
        "msg" => {
            if args.len() < 3 {
                "Usage: ttv msg <channel> <message text...>".to_string()
            } else {
                let channel = args[1];
                let text = args[2..].join(" ");
                do_send_message(channel, &text, bot_api, tui_module).await
            }
        }
        "chat" => {
            // Enter chat mode
            let mut tm = tui_module.ttv_state.lock().unwrap();
            if tm.joined_channels.is_empty() {
                "No channels joined. Use 'ttv join <channel>' first.".to_string()
            } else {
                tm.is_in_chat_mode = true;
                tm.current_channel_index = 0;
                format!(
                    "Chat mode enabled. Type '/quit' to exit. Current channel: {}",
                    tm.joined_channels[0]
                )
            }
        }
        "default" => {
            if args.len() < 2 {
                "Usage: ttv default <channelName>".to_string()
            } else {
                set_default_channel(args[1], bot_api, tui_module).await
            }
        }
        _ => "Unrecognized ttv subcommand. Type `ttv` for usage.".to_string(),
    }
}

fn set_active_account(account: &str, tui_module: &Arc<TuiModule>) -> String {
    let mut st = tui_module.ttv_state.lock().unwrap();
    st.active_account = account.to_string();
    format!("Active Twitch account set to '{}'", account)
}

/// Joins the specified channel with the *active* TTV account, ensuring the Twitch-IRC runtime is started.
async fn do_join_channel(
    channel: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = normalize_channel_name(channel);

    // Gather needed info from ttv_state without holding the lock across awaits
    let (already_joined, active_account) = {
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
        return format!("Already joined channel '{}'.", chname);
    }

    // Now do the async calls safely, outside the lock
    if let Err(e) = bot_api.start_platform_runtime("twitch-irc", &active_account).await {
        return format!("Error starting twitch-irc => {:?}", e);
    }
    match bot_api.join_twitch_irc_channel(&active_account, &chname).await {
        Ok(_) => format!("Joined channel '{}'. Now receiving messages.", chname),
        Err(e) => format!("Error joining '{}': {:?}", chname, e),
    }
}

/// Parts the specified channel with the *active* TTV account
async fn do_part_channel(
    channel: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = normalize_channel_name(channel);

    // Lock briefly
    let (pos_opt, active_account) = {
        let mut st = tui_module.ttv_state.lock().unwrap();
        let pos = st.joined_channels
            .iter()
            .position(|c| c.eq_ignore_ascii_case(&chname));
        (pos, st.active_account.clone())
    };

    // If we never joined that channel, bail now
    if pos_opt.is_none() {
        return format!("Not currently joined to '{}'.", chname);
    }

    // Actually remove from joined_channels in a separate pass
    // so we don't hold the lock across the async call
    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        if let Some(idx) = pos_opt {
            st.joined_channels.remove(idx);
        }
    }

    // Now do the async part
    match bot_api.part_twitch_irc_channel(&active_account, &chname).await {
        Ok(_) => format!("Parted channel '{}'.", chname),
        Err(e) => format!("Error parting channel '{}': {:?}", chname, e),
    }
}

/// Sends a message to a channel using the *active* TTV account
async fn do_send_message(
    channel: &str,
    text: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = normalize_channel_name(channel);

    // Copy out the active account from the TUI state
    let active_acc = {
        let st = tui_module.ttv_state.lock().unwrap();
        st.active_account.clone()
    };

    // Then do async call outside the lock
    match bot_api.send_twitch_irc_message(&active_acc, &chname, text).await {
        Ok(_) => format!("[{}] {}: {}", chname, active_acc, text),
        Err(e) => format!("Error sending msg to '{}': {:?}", chname, e),
    }
}

/// Sets the "default channel" in memory and in bot_config, so it’s auto-joined on restart.
async fn set_default_channel(
    channel: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = normalize_channel_name(channel);

    // Just store in TUI memory
    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.default_channel = Some(chname.clone());
    }

    // Also store in bot_config
    match bot_api.set_bot_config_value("ttv_default_channel", &chname).await {
        Ok(_) => format!("Default channel set to '{}'. Will auto-join on restart.", chname),
        Err(e) => format!("Error storing default channel => {:?}", e),
    }
}

/// If user typed `mychan`, convert to `#mychan`
fn normalize_channel_name(chan: &str) -> String {
    let c = chan.trim();
    if c.starts_with('#') {
        c.to_string()
    } else {
        format!("#{}", c)
    }
}