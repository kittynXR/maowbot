use std::sync::Arc;
use std::io::{stdout, Write};
use tokio::runtime::Handle;

use maowbot_core::Error;
use maowbot_core::plugins::bot_api::BotApi;

use uuid::Uuid;

use crate::tui_module::TuiModule;

/// Dispatches subcommands for `ttv`:
///
///  - `ttv active <account>`
///  - `ttv join <channel>`
///  - `ttv part <channel>`
///  - `ttv msg <channel> <message text>`
///  - `ttv chat`
///  - `ttv default <channel>`
///
/// By default, the "active" Twitch account is the broadcaster account
/// (assume you only have one main broadcaster). If the user issues
/// `ttv active <account>` then they pick one of the **bot** accounts
/// to become active.
///
/// "join", "part", "msg", "chat", and "default" all operate on the **active** TTV account.
///
/// - `join` => Start following that channel’s messages in the TUI (and actually join the IRC).
/// - `part` => Stop following that channel’s messages (leave the IRC).
/// - `msg` => Send a chat message to the given channel.
/// - `chat` => Enter chat mode; the TUI prompt changes from `tui>` to `#channel>`.
/// - `default` => Set the default channel that will be auto-joined on restart.
///
/// If the user restarts the bot, only the default channel is joined automatically (others are lost).
/// If the user joined multiple channels, the TUI should show messages from all joined channels,
/// but entering “chat mode” will focus sending to a single channel at a time.
/// The user can type `/c` in chat mode to cycle among joined channels.
///
/// Type `/quit` in chat mode to leave chat mode and go back to `tui>` prompt.
pub fn handle_ttv_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return r#"Usage:
  ttv active <accountName>
  ttv join <channel>
  ttv part <channel>
  ttv msg <channel> <text>
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
                do_join_channel(args[1], bot_api, tui_module)
            }
        }
        "part" => {
            if args.len() < 2 {
                "Usage: ttv part <channelName>".to_string()
            } else {
                do_part_channel(args[1], bot_api, tui_module)
            }
        }
        "msg" => {
            if args.len() < 3 {
                "Usage: ttv msg <channel> <message text...>".to_string()
            } else {
                let channel = args[1];
                let text = args[2..].join(" ");
                do_send_message(channel, &text, bot_api, tui_module)
            }
        }
        "chat" => {
            // Enter chat mode on the TUI
            let mut tm = tui_module.ttv_state.lock().unwrap();
            if tm.joined_channels.is_empty() {
                "No channels joined. Join a channel first (ttv join <channel>).".to_string()
            } else {
                tm.is_in_chat_mode = true;
                tm.current_channel_index = 0; // Start with first joined channel
                format!("Chat mode enabled. Type '/quit' to exit. Current channel: {}",
                        tm.joined_channels[0])
            }
        }
        "default" => {
            if args.len() < 2 {
                "Usage: ttv default <channelName>".to_string()
            } else {
                set_default_channel(args[1], bot_api, tui_module)
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

fn do_join_channel(channel: &str, bot_api: &Arc<dyn BotApi>, tui_module: &Arc<TuiModule>) -> String {
    let chname = normalize_channel_name(channel);
    let st = {
        let mut s = tui_module.ttv_state.lock().unwrap();
        // If already joined, do nothing
        if s.joined_channels.iter().any(|c| c.eq_ignore_ascii_case(&chname)) {
            return format!("Already joined channel '{}'.", chname);
        }
        s.joined_channels.push(chname.clone());
        // Also record for TUI chat filters
        s.joined_channels.sort();
        s.active_account.clone()
    };

    // We must ensure the "twitch-irc" platform for that account is actually started:
    // (i.e., start the twitch-irc runtime for the active account)
    let rt_res = Handle::current().block_on(async {
        bot_api.start_platform_runtime("twitch-irc", &st).await
    });
    if let Err(e) = rt_res {
        return format!("Error starting twitch-irc for account='{}': {:?}", st, e);
    }

    // Then tell it to join the channel
    let join_res = Handle::current().block_on(async {
        bot_api.join_twitch_irc_channel(&st, &chname).await
    });
    match join_res {
        Ok(_) => format!("Joined channel '{}'. Now receiving messages.", chname),
        Err(e) => format!("Error joining '{}': {:?}", chname, e),
    }
}

fn do_part_channel(channel: &str, bot_api: &Arc<dyn BotApi>, tui_module: &Arc<TuiModule>) -> String {
    let chname = normalize_channel_name(channel);
    let active_acc = {
        let mut st = tui_module.ttv_state.lock().unwrap();
        if let Some(pos) = st.joined_channels.iter().position(|c| c.eq_ignore_ascii_case(&chname)) {
            st.joined_channels.remove(pos);
        } else {
            return format!("Not currently joined to '{}'.", chname);
        }
        st.active_account.clone()
    };

    // Instruct the IRC platform to leave that channel
    let part_res = Handle::current().block_on(async {
        bot_api.part_twitch_irc_channel(&active_acc, &chname).await
    });
    match part_res {
        Ok(_) => format!("Parted channel '{}'.", chname),
        Err(e) => format!("Error parting channel '{}': {:?}", chname, e),
    }
}

fn do_send_message(
    channel: &str,
    text: &str,
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    let chname = normalize_channel_name(channel);
    let active_acc = {
        let st = tui_module.ttv_state.lock().unwrap();
        st.active_account.clone()
    };

    let send_res = Handle::current().block_on(async {
        bot_api.send_twitch_irc_message(&active_acc, &chname, text).await
    });
    match send_res {
        Ok(_) => format!("[{}] {}: {}", chname, active_acc, text),
        Err(e) => format!("Error sending msg to '{}': {:?}", chname, e),
    }
}

fn set_default_channel(channel: &str, bot_api: &Arc<dyn BotApi>, tui_module: &Arc<TuiModule>) -> String {
    let chname = normalize_channel_name(channel);
    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.default_channel = Some(chname.clone());
    }

    // Optionally persist to bot_config so it is remembered next restart.
    // E.g. store under "ttv_default_channel" with the active account if needed.
    // For a single broadcaster, you might do:
    let set_res = Handle::current().block_on(async {
        bot_api.set_bot_config_value("ttv_default_channel", &chname).await
    });
    match set_res {
        Ok(_) => format!("Default channel set to '{}'. Will auto-join on restart.", chname),
        Err(e) => format!("Error storing default channel => {:?}", e),
    }
}

/// Normalizes a channel name so it has the "#" prefix.
/// If user typed "mychannel", we become "#mychannel".
/// If they typed "#mychannel", we leave it as-is.
fn normalize_channel_name(chan: &str) -> String {
    let c = chan.trim();
    if c.starts_with('#') {
        c.to_string()
    } else {
        format!("#{}", c)
    }
}
