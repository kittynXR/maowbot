// maowbot-tui/src/commands/discord.rs
//
// Implements TUI subcommands for Discord. Now uses real data via the new DiscordApi-ish
// methods on BotApi (e.g. list_discord_guilds, list_discord_channels, etc.).
//
// The calls like `bot_api.get_discord_active_server(...)` return Result<Option<String>, Error>,
// which is the active server ID for a given Discord account.
//
// In your codebase, if you want an Arc<dyn DiscordApi>, you would add a method on BotApi to
// retrieve it. For now, we assume you have direct calls to e.g. bot_api.list_discord_guilds(...),
// set_discord_active_server(...), etc., which do not require a separate 'discord_api()' method.

use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};

use maowbot_common::traits::api::BotApi;
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
        "help" => show_discord_usage(),

        "active" => {
            if args.len() < 2 {
                return "Usage:\n  discord active server [guildId]\n".to_string();
            }
            if args[1].eq_ignore_ascii_case("server") {
                let maybe_guild_id = args.get(2).map(|s| s.to_string());
                set_or_select_discord_server(bot_api, maybe_guild_id).await
            } else {
                "Usage: discord active server <guildId>?".to_string()
            }
        }

        "list" => {
            // usage: `discord list guilds [accountName]?`
            //        `discord list channels [accountName]? [guildId]?`
            if args.len() < 2 {
                return "Usage: discord list (guilds|channels) [accountName]? [guildId]?".to_string();
            }
            let sub = args[1].to_lowercase();
            if sub == "guilds" {
                list_discord_guilds(bot_api, &args[2..]).await
            } else if sub == "channels" {
                list_discord_channels(bot_api, &args[2..]).await
            } else {
                format!("Unknown 'list' target '{}'", sub)
            }
        }

        "chat" => {
            if args.len() < 2 {
                return "Usage: discord chat <channelId>".to_string();
            }
            open_discord_chat_mode(bot_api, args[1]).await
        }

        _ => show_discord_usage(),
    }
}

fn show_discord_usage() -> String {
    r#"Usage: discord <subcommand> ...
Subcommands:
  discord help
  discord active server [guildId]
  discord list guilds [accountName]?
  discord list channels [accountName]? [guildId]?
  discord chat <channelId>
"#
        .to_string()
}

/// Sets the active server for the “discord_active_account” if a guildId is given;
/// otherwise lists guilds and prompts the user to pick one.
async fn set_or_select_discord_server(
    bot_api: &Arc<dyn BotApi>,
    maybe_guild_id: Option<String>
) -> String {
    // 1) read the account name from bot_config
    let account_name = match bot_api.get_bot_config_value("discord_active_account").await {
        Ok(Some(v)) => v,
        _ => return "No 'discord_active_account' is set in bot_config.".to_string(),
    };

    // 2) If the user typed a guild ID right in the command, set it:
    if let Some(gid) = maybe_guild_id {
        match bot_api.set_discord_active_server(&account_name, &gid).await {
            Ok(_) => format!("Active server set to {} for account '{}'", gid, account_name),
            Err(e) => format!("Error setting active server => {:?}", e),
        }
    } else {
        // Otherwise, we list known guilds and prompt user to pick
        let guilds = match bot_api.list_discord_guilds(&account_name).await {
            Ok(g) => g,
            Err(e) => return format!("Error listing guilds => {e:?}"),
        };
        if guilds.is_empty() {
            return format!(
                "No guilds found for account '{account_name}'. Bot may not be joined to any servers."
            );
        }

        println!("Known guilds for account '{account_name}':");
        for (i, g) in guilds.iter().enumerate() {
            println!("  [{}] {} (ID={})", i + 1, g.guild_name, g.guild_id);
        }
        print!("Pick a guild number to set active: ");
        let mut input_line = String::new();
        let mut stdin = BufReader::new(io::stdin());
        if stdin.read_line(&mut input_line).await.is_err() {
            return "Failed to read from stdin.".to_string();
        }
        let trimmed = input_line.trim();
        if trimmed.is_empty() {
            return "Cancelled; no input.".to_string();
        }
        let idx = match trimmed.parse::<usize>() {
            Ok(n) => n,
            Err(_) => 0,
        };
        if idx < 1 || idx > guilds.len() {
            return "Invalid selection.".to_string();
        }
        let chosen = &guilds[idx - 1];
        match bot_api
            .set_discord_active_server(&account_name, &chosen.guild_id)
            .await
        {
            Ok(_) => format!(
                "Active server set to {} for account '{}'",
                chosen.guild_id, account_name
            ),
            Err(e) => format!("Error setting active server => {e:?}"),
        }
    }
}

/// Lists guilds for either the “discord_active_account” or a user-specified account.
async fn list_discord_guilds(bot_api: &Arc<dyn BotApi>, args: &[&str]) -> String {
    // If user gave an accountName, use it. Otherwise read from "discord_active_account".
    let account_name = if let Some(a) = args.get(0) {
        a.to_string()
    } else {
        match bot_api.get_bot_config_value("discord_active_account").await {
            Ok(Some(v)) => v,
            _ => return "Please specify an accountName or set 'discord_active_account'.".to_string(),
        }
    };

    let guilds = match bot_api.list_discord_guilds(&account_name).await {
        Ok(g) => g,
        Err(e) => return format!("Error listing guilds => {e:?}"),
    };
    if guilds.is_empty() {
        return format!("No guilds found for account '{account_name}'.");
    }
    let mut out = format!("Guilds for account '{account_name}':\n");
    for g in guilds {
        out.push_str(&format!(" - {} (ID={})\n", g.guild_name, g.guild_id));
    }
    out
}

/// Lists channels for a given guild. If user doesn't specify a guild, we read the “active” one
/// from the DB. The user can also specify accountName or rely on "discord_active_account".
async fn list_discord_channels(bot_api: &Arc<dyn BotApi>, args: &[&str]) -> String {
    // usage: discord list channels [acct]? [guildId]?

    let (account_name, maybe_guild_id) = match args.len() {
        0 => {
            // read from config
            let acct_opt = bot_api.get_bot_config_value("discord_active_account").await;
            let Some(acct) = acct_opt.ok().flatten() else {
                return "No 'discord_active_account' set. Provide an account or set one.".into();
            };
            // read its active server
            match bot_api.get_discord_active_server(&acct).await {
                Ok(Some(gid)) => (acct, Some(gid)),
                Ok(None) => {
                    return format!(
                        "No active server set for account '{acct}'. Provide a guildId or run 'discord active server'."
                    );
                }
                Err(e) => return format!("Error fetching active server => {e:?}"),
            }
        }
        1 => {
            // single arg => interpret it as an accountName, then read that account's active server
            let acct = args[0].to_string();
            match bot_api.get_discord_active_server(&acct).await {
                Ok(Some(gid)) => (acct, Some(gid)),
                Ok(None) => {
                    return format!(
                        "No active server set for account '{acct}'. Provide a guildId or run 'discord active server'."
                    );
                }
                Err(e) => return format!("Error fetching active server => {e:?}"),
            }
        }
        _ => {
            // 2 or more => interpret first as accountName, second as guildId
            let acct = args[0].to_string();
            let gid = args[1].to_string();
            (acct, Some(gid))
        }
    };

    let guild_id = match maybe_guild_id {
        Some(g) => g,
        None => {
            return format!(
                "No guild ID found or set for account '{account_name}'."
            );
        }
    };

    let channels = match bot_api.list_discord_channels(&account_name, &guild_id).await {
        Ok(c) => c,
        Err(e) => return format!("Error listing channels => {e:?}"),
    };
    if channels.is_empty() {
        return format!(
            "No channels found for account='{account_name}', guild_id='{guild_id}'."
        );
    }

    let mut out = format!("Channels for account='{}', guild='{}':\n", account_name, guild_id);
    for ch in channels {
        out.push_str(&format!(" - {} (ID={})\n", ch.channel_name, ch.channel_id));
    }
    out
}

/// Opens a mini text REPL to send messages to a given channel. `_bot_api` is not used yet but
/// is kept in the signature if you eventually want to call send_discord_message.
async fn open_discord_chat_mode(
    _bot_api: &Arc<dyn BotApi>,
    channel_id: &str
) -> String {
    println!("(Discord chat mode) Type '/quit' to exit. Sending to channel '{channel_id}'...");
    let mut stdin = BufReader::new(io::stdin());

    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).await.is_err() {
            return "(Error reading stdin)".to_string();
        }
        let text = line.trim();
        if text.eq_ignore_ascii_case("/quit") {
            return "Exiting Discord chat mode.".to_string();
        }

        // If you had a method like _bot_api.send_discord_message(accountName, channel_id, text),
        // you could call it here. For now, we just emulate:
        println!("(you => {channel_id}): {text}");
    }
}
