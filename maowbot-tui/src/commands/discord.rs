// File: maowbot-tui/src/commands/discord.rs

use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use maowbot_common::traits::api::{BotApi, BotConfigApi};
use crate::tui_module::TuiModule;

pub async fn handle_discord_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    _tui: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return show_discord_usage();
    }

    match args[0].to_lowercase().as_str() {
        "active" => {
            if args.len() < 2 {
                return "Usage: discord active <discordusername>".to_string();
            }
            set_active_discord_account(bot_api, args[1]).await
        }

        "server" => {
            // If no serverid provided => list them
            let maybe_srv = args.get(1).map(|x| x.to_string());
            set_or_select_discord_server(bot_api, maybe_srv).await
        }

        "chat" => {
            if args.len() < 2 {
                return "Usage: discord chat <channel>".to_string();
            }
            open_discord_chat_mode(bot_api, args[1]).await
        }

        "list" => {
            // possible sub-subcommand: "channels"?
            if args.get(1).map(|x| x.to_lowercase()) == Some("channels".to_string()) {
                list_discord_channels(bot_api).await
            } else {
                "Usage: discord list channels".to_string()
            }
        }

        "setchannel" => {
            if args.len() < 3 {
                return "Usage: discord setchannel <keyname> <channelname>".to_string();
            }
            let keyname = args[1];
            let channelname = args[2..].join(" ");
            set_discord_channel_alias(bot_api, keyname, &channelname).await
        }

        _ => show_discord_usage(),
    }
}

fn show_discord_usage() -> String {
    r#"Usage:
  discord               # display this usage text
  discord active <username>
       # sets the given discord username as the "active" account, stored in bot_config
       #   (config_key="discord", config_value="active_account", config_meta: optional)

  discord server [serverid]
       # sets the active discord server. if no serverid given, list currently joined servers
       #   and let user pick e.g. 1, 2, 3.

  discord chat <channel>
       # opens a REPL-like interface to send messages to <channel> as the active account
       #   use '/quit' to exit.

  discord list channels
       # lists all channels with the active discord account. If multiple servers,
       #   ask for numeric input to pick which server to show.

  discord setchannel <keyname> <channelname>
       # sets an active channel that corresponds to <keyname> in the code
       # e.g. "discord setchannel announcements #general"
    "#.to_string()
}

/// Example: store in bot_config -> (config_key="discord", config_value="active_account"), meta = null
async fn set_active_discord_account(bot_api: &Arc<dyn BotApi>, username: &str) -> String {
    let res = bot_api.set_bot_config_value( "discord_active_account", username).await;
    if let Err(e) = res {
        return format!("Error setting active discord account => {e:?}");
    }
    format!("Discord active account set to '{username}'")
}

/// If serverid is provided, store it. Otherwise, list servers and prompt user.
async fn set_or_select_discord_server(bot_api: &Arc<dyn BotApi>, maybe_srv: Option<String>) -> String {
    if let Some(srv) = maybe_srv {
        // user typed "discord server 123456789"
        let res = bot_api.set_bot_config_value("discord_active_server", &srv).await;
        return match res {
            Ok(_) => format!("Active Discord server set to {srv}"),
            Err(e) => format!("Error setting active server => {e:?}"),
        };
    }

    // otherwise, list joined servers and let user pick
    // (for demonstration, we just pretend we have a list)
    let dummy_servers = vec!["11111", "22222", "33333"];
    let mut output = String::new();
    output.push_str("Currently joined servers:\n");
    for (i, srv) in dummy_servers.iter().enumerate() {
        output.push_str(&format!("  [{}] {}\n", i + 1, srv));
    }
    output.push_str("Type a number to select a server.\n");

    // We'll do a quick read from stdin in-line:
    output.push_str("> ");
    println!("{}", output);

    let mut input_line = String::new();
    let mut stdin = BufReader::new(io::stdin());
    if stdin.read_line(&mut input_line).await.is_err() {
        return "(Error reading input)".to_string();
    }
    let choice = input_line.trim().parse::<usize>().unwrap_or(0);
    if choice == 0 || choice > dummy_servers.len() {
        return "Invalid choice. Cancelled.".to_string();
    }
    let chosen_id = dummy_servers[choice - 1].to_string();

    let res = bot_api.set_bot_config_value("discord_active_server", &chosen_id).await;
    match res {
        Ok(_) => format!("Active Discord server set to {}", chosen_id),
        Err(e) => format!("Error setting server => {e:?}"),
    }
}

/// Opens a REPL-like interface for Discord chat.
/// For demonstration, we just do local reading,
/// since in practice you'd call `bot_api` to send messages to the channel.
async fn open_discord_chat_mode(bot_api: &Arc<dyn BotApi>, channel: &str) -> String {
    // For demonstration, a minimal "while" loop.
    // Real code might set some state in TuiModule like the twitch chat approach.
    let channel_str = channel.to_string();
    println!("(Discord Chat) Type '/quit' to exit. Sending to channel '{channel_str}'...");

    let mut stdin = BufReader::new(io::stdin());
    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).await.is_err() {
            return "Error reading from stdin.".to_string();
        }
        let text = line.trim();
        if text.eq_ignore_ascii_case("/quit") {
            return "Exiting Discord chat mode.".to_string();
        }

        // Here, you'd do something like:
        //  bot_api.send_discord_message(active_account, &channel_str, text).await
        // But your BotApi does not (yet) have that method in this example code.
        println!("(Discord) [You => {channel_str}] {text}");
    }
}

/// Lists all channels for the active Discord account.
/// If joined to multiple servers, we display a list and ask user to pick.
async fn list_discord_channels(bot_api: &Arc<dyn BotApi>) -> String {
    // For now, just a dummy example
    let dummy_channels = vec!["#general", "#random", "#bot-stuff"];
    let mut output = String::new();
    output.push_str("Channels in active server:\n");
    for (i, chan) in dummy_channels.iter().enumerate() {
        output.push_str(&format!("  [{}] {}\n", i + 1, chan));
    }
    output
}

/// Sets a “key => channelname” in config, so your code can look up the channel for e.g. announcements.
async fn set_discord_channel_alias(bot_api: &Arc<dyn BotApi>, keyname: &str, channelname: &str) -> String {
    // For example, we do: (config_key="discord_channels", config_value=<keyname>, config_meta= { "channel": <channelname> } )
    let meta = serde_json::json!({ "channel": channelname });
    let res = bot_api.set_config_kv_meta("discord_channels", keyname, Some(meta)).await;
    match res {
        Ok(_) => format!("Set Discord channel alias '{keyname}' => '{channelname}'"),
        Err(e) => format!("Error setting alias => {e:?}"),
    }
}
