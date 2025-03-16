use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};

use maowbot_common::traits::api::BotApi;
use maowbot_common::error::Error;

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
                return show_discord_usage();
            }
            match args[1].to_lowercase().as_str() {
                "account" => {
                    let maybe_name = args.get(2).map(|s| s.to_string());
                    set_or_select_discord_account(bot_api, maybe_name).await
                }
                "server" => {
                    let maybe_guild = args.get(2).map(|s| s.to_string());
                    set_or_select_discord_server(bot_api, maybe_guild).await
                }
                "channel" => {
                    let maybe_channel = args.get(2).map(|s| s.to_string());
                    set_or_select_discord_channel(bot_api, maybe_channel).await
                }
                _ => show_discord_usage(),
            }
        }

        "list" => {
            if args.len() < 2 {
                return "Usage: discord list (guilds|channels) [accountName]? [guildId]?".to_string();
            }
            let sub = args[1].to_lowercase();
            if sub == "guilds" {
                list_discord_guilds(bot_api, &args[2..]).await
            } else if sub == "channels" {
                list_discord_channels(bot_api, &args[2..]).await
            } else {
                format!("Unknown 'list' target '{sub}'")
            }
        }

        "chat" => {
            if args.len() < 4 {
                return "Usage: discord chat <accountName> <guildId> <channelId>".to_string();
            }
            let acct = args[1];
            let gid = args[2];
            let cid = args[3];
            open_discord_chat_mode(bot_api, acct, gid, cid).await
        }
        _ => show_discord_usage(),
    }
}

fn show_discord_usage() -> String {
    r#"Usage: discord <subcommand> ...
Subcommands:
  discord help
  discord active account [accountName]?
  discord active server [guildId]?
  discord active channel [channelId]?
  discord list guilds [accountName]?
  discord list channels [guildId]?
  discord chat [accountName] [guildId] [channelId]
  discord sync [accountName]?
"#
        .to_string()
}

async fn set_or_select_discord_account(
    bot_api: &Arc<dyn BotApi>,
    maybe_acct: Option<String>,
) -> String {
    if let Some(name) = maybe_acct {
        match bot_api.set_discord_active_account(&name).await {
            Ok(_) => format!("Discord active account set to '{}'", name),
            Err(e) => format!("Error setting active account => {e:?}"),
        }
    } else {
        let accounts = match bot_api.list_discord_accounts().await {
            Ok(a) => a,
            Err(e) => return format!("Error listing discord accounts => {e:?}"),
        };
        if accounts.is_empty() {
            return "No Discord accounts found.".to_string();
        }

        println!("Known Discord accounts:");
        for (i, acct) in accounts.iter().enumerate() {
            let marker = if acct.is_active { "*" } else { " " };
            println!("  [{}] {}  (active={})", i + 1, acct.account_name, marker);
        }
        print!("Pick an account number: ");
        let mut line = String::new();
        let mut stdin = BufReader::new(io::stdin());
        if stdin.read_line(&mut line).await.is_err() {
            return "(Failed to read)".to_string();
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return "Cancelled.".to_string();
        }
        let idx = match trimmed.parse::<usize>() {
            Ok(x) => x,
            Err(_) => 0,
        };
        if idx < 1 || idx > accounts.len() {
            return "Invalid selection.".to_string();
        }
        let chosen = &accounts[idx - 1];
        match bot_api.set_discord_active_account(&chosen.account_name).await {
            Ok(_) => format!("Discord active account set to '{}'", chosen.account_name),
            Err(e) => format!("Error setting active account => {e:?}"),
        }
    }
}

async fn set_or_select_discord_server(
    bot_api: &Arc<dyn BotApi>,
    maybe_gid: Option<String>,
) -> String {
    let account_name = match bot_api.get_discord_active_account().await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return "No active Discord account set. Run 'discord active account' first.".to_string();
        }
        Err(e) => return format!("Error => {e:?}"),
    };

    if let Some(gid) = maybe_gid {
        match bot_api.set_discord_active_server(&account_name, &gid).await {
            Ok(_) => format!("Active server set to {gid} for account '{account_name}'"),
            Err(e) => format!("Error setting active server => {e:?}"),
        }
    } else {
        let guilds = match bot_api.list_discord_guilds(&account_name).await {
            Ok(g) => g,
            Err(e) => return format!("Error listing guilds => {e:?}"),
        };
        if guilds.is_empty() {
            return format!("No guilds found for account '{account_name}'.");
        }
        println!("Known guilds for account '{account_name}':");
        for (i, g) in guilds.iter().enumerate() {
            let marker = if g.is_active { "*" } else { " " };
            println!(
                "  [{}] {} (ID={}), active={}",
                i + 1,
                g.guild_name,
                g.guild_id,
                marker
            );
        }
        print!("Pick a guild number: ");
        let mut line = String::new();
        let mut stdin = BufReader::new(io::stdin());
        if stdin.read_line(&mut line).await.is_err() {
            return "Failed to read from stdin.".to_string();
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return "Cancelled.".to_string();
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
            Err(e) => format!("Error => {e:?}"),
        }
    }
}

async fn set_or_select_discord_channel(
    bot_api: &Arc<dyn BotApi>,
    maybe_cid: Option<String>,
) -> String {
    let account_name = match bot_api.get_discord_active_account().await {
        Ok(Some(a)) => a,
        Ok(None) => return "No active account. Run 'discord active account' first.".to_string(),
        Err(e) => return format!("Error => {e:?}"),
    };
    let guild_id = match bot_api.get_discord_active_server(&account_name).await {
        Ok(Some(gid)) => gid,
        Ok(None) => {
            return format!(
                "No active server found for account '{account_name}'. Use 'discord active server' first."
            );
        }
        Err(e) => return format!("Error => {e:?}"),
    };

    if let Some(cid) = maybe_cid {
        match bot_api.set_discord_active_channel(&account_name, &guild_id, &cid).await {
            Ok(_) => format!("Active channel set to {cid} for guild='{guild_id}'"),
            Err(e) => format!("Error setting active channel => {e:?}"),
        }
    } else {
        let channels = match bot_api.list_discord_channels(&account_name, &guild_id).await {
            Ok(c) => c,
            Err(e) => return format!("Error listing channels => {e:?}"),
        };
        if channels.is_empty() {
            return format!(
                "No channels found in guild='{guild_id}' for account='{account_name}'."
            );
        }

        println!("Channels in guild='{guild_id}':");
        for (i, c) in channels.iter().enumerate() {
            let marker = if c.is_active { "*" } else { " " };
            println!(
                "  [{}] {} (ID={}), active={}",
                i + 1,
                c.channel_name,
                c.channel_id,
                marker
            );
        }
        print!("Pick a channel number: ");
        let mut line = String::new();
        let mut stdin = BufReader::new(io::stdin());
        if stdin.read_line(&mut line).await.is_err() {
            return "Failed to read from stdin.".to_string();
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return "Cancelled.".to_string();
        }
        let idx = match trimmed.parse::<usize>() {
            Ok(n) => n,
            Err(_) => 0,
        };
        if idx < 1 || idx > channels.len() {
            return "Invalid selection.".to_string();
        }
        let chosen = &channels[idx - 1];
        match bot_api
            .set_discord_active_channel(&account_name, &guild_id, &chosen.channel_id)
            .await
        {
            Ok(_) => format!(
                "Active channel set to {} (named '{}')",
                chosen.channel_id, chosen.channel_name
            ),
            Err(e) => format!("Error => {e:?}"),
        }
    }
}

async fn list_discord_guilds(bot_api: &Arc<dyn BotApi>, args: &[&str]) -> String {
    let account_name = if let Some(a) = args.get(0) {
        a.to_string()
    } else {
        match bot_api.get_discord_active_account().await {
            Ok(Some(acct)) => acct,
            Ok(None) => return "No active Discord account and none specified.".to_string(),
            Err(e) => return format!("Error => {e:?}"),
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
        out.push_str(&format!(
            " - {} (ID={}), active={}\n",
            g.guild_name, g.guild_id, g.is_active
        ));
    }
    out
}

async fn list_discord_channels(bot_api: &Arc<dyn BotApi>, args: &[&str]) -> String {
    let account_name = match bot_api.get_discord_active_account().await {
        Ok(Some(acct)) => acct,
        Ok(None) => {
            return "No active Discord account. Provide an account or set one via 'discord active account'.".to_string();
        }
        Err(e) => return format!("Error => {e:?}"),
    };

    let maybe_guild_id = if let Some(g) = args.get(0) {
        Some(g.to_string())
    } else {
        match bot_api.get_discord_active_server(&account_name).await {
            Ok(Some(gid)) => Some(gid),
            Ok(None) => {
                return format!(
                    "No active guild for account '{account_name}'. Provide a guildId or set one."
                );
            }
            Err(e) => return format!("Error => {e:?}"),
        }
    };

    let guild_id = match maybe_guild_id {
        Some(gid) => gid,
        None => {
            return "No guild ID found or set. Provide one or set an active server.".to_string();
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

    let mut out = format!("Channels in guild='{guild_id}' for account='{account_name}':\n");
    for ch in channels {
        out.push_str(&format!(
            " - {} (ID={}), active={}\n",
            ch.channel_name, ch.channel_id, ch.is_active
        ));
    }
    out
}

async fn open_discord_chat_mode(
    bot_api: &Arc<dyn BotApi>,
    account_name: &str,
    guild_id: &str,
    channel_id: &str
) -> String {
    println!("(Discord chat mode) account='{account_name}', guild='{guild_id}', channel='{channel_id}'");
    println!("Type '/quit' to exit. Type anything else to send a message (demo).");

    let _ = bot_api.start_platform_runtime("discord", account_name).await.map_err(|e| {
        eprintln!("Failed to start Discord runtime => {e:?}");
    });

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

        // Real usage would be: bot_api.send_discord_message(...). We skip that for now:
        println!("(You => #{}): {}", channel_id, text);
    }
}
