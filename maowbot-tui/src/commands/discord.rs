// File: maowbot-tui/src/commands/discord.rs
//
// Implements the TUI subcommands for Discord:
//
//   discord
//     -> displays usage text
//
//   discord active user <discordusername>
//     -> sets the active Discord account, stored in bot_config key="discord_active_account"
//
//   discord active server [serverid]
//     -> sets the active server in JSON metadata for that account. If no serverid is provided,
//        list the joined servers and let the user pick. We store it in:
//          set_config_kv_meta("discord", <accountName>, { "active_server": "...", ... })
//
//   discord list [accountname] [servername] channels
//     -> lists channels for the given account & server. If omitted, use the "active" account & server
//
//   discord set [accountname] [servername] <keyname> <value>
//     -> sets a key-value in the JSON metadata for the given account (and optionally server).
//        If no account/server is provided, we use the active ones from bot_config.
//
//   discord chat <channel>
//     -> opens a REPL-like interface to send messages to <channel> as the active Discord account.
//        type `/quit` to exit.
//
// Usage: type “discord” or “discord <subcommand> ...”

use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use serde_json::json;

use maowbot_common::traits::api::{BotApi, BotConfigApi};
use crate::tui_module::TuiModule;

/// Main dispatcher for the “discord” command in the TUI.
pub async fn handle_discord_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    _tui: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return show_discord_usage();
    }

    match args[0].to_lowercase().as_str() {
        // ------------------------------------------------------------------
        // "discord" => just show usage
        // ------------------------------------------------------------------
        "help" => show_discord_usage(),

        // ------------------------------------------------------------------
        // "discord active user <discordusername>"
        // ------------------------------------------------------------------
        "active" => {
            if args.len() < 2 {
                // "discord active" by itself => show usage
                return r#"Usage:
  discord active user <discordusername>
  discord active server [serverid]
"#.to_string();
            }
            match args[1].to_lowercase().as_str() {
                "user" => {
                    if args.len() < 3 {
                        return "Usage: discord active user <discordusername>".to_string();
                    }
                    set_active_discord_account(bot_api, args[2]).await
                }
                "server" => {
                    // usage: "discord active server <serverid>?" => if no <serverid>, prompt
                    let maybe_server = args.get(2).map(|s| s.to_string());
                    set_or_select_discord_server(bot_api, maybe_server).await
                }
                _ => "Invalid usage of 'discord active'. Try 'discord active user <name>' or 'discord active server'.".to_string(),
            }
        }

        // ------------------------------------------------------------------
        // "discord list [accountname] [servername] channels"
        // ------------------------------------------------------------------
        "list" => {
            // Minimal usage: "discord list" => usage text
            // Full usage:  "discord list [acct] [server] channels"
            //
            // We’ll parse to see if "channels" is the last token
            if args.len() >= 1 && args.last().map(|s| s.to_lowercase()) == Some("channels".to_string()) {
                list_discord_channels(bot_api, &args[1..]).await
            } else {
                "Usage: discord list [accountname] [servername] channels".to_string()
            }
        }

        // ------------------------------------------------------------------
        // "discord set [accountname] [servername] <keyname> <value>"
        // ------------------------------------------------------------------
        "set" => {
            if args.len() < 3 {
                return r#"Usage: discord set [accountname] [servername] <keyname> <value>

If you omit accountname and servername, we use the "active" ones from bot_config."#.to_string();
            }
            set_discord_metadata(bot_api, args).await
        }

        // ------------------------------------------------------------------
        // "discord chat <channel>"
        // ------------------------------------------------------------------
        "chat" => {
            if args.len() < 2 {
                return "Usage: discord chat <channel>".to_string();
            }
            let channel = args[1];
            open_discord_chat_mode(bot_api, channel).await
        }

        // ------------------------------------------------------------------
        // If unrecognized subcommand or partial usage:
        // ------------------------------------------------------------------
        _ => show_discord_usage(),
    }
}

/// Displays short usage text for the “discord” command.
fn show_discord_usage() -> String {
    r#"Usage: discord <subcommand> [options...]

Subcommands:
  discord              # displays this usage text

  discord active user <discordusername>
      # sets the active Discord account. Stored in bot_config key="discord_active_account"

  discord active server <serverid>?
      # sets the active Discord server for the active account. If no serverid is given,
      # lists the user's joined servers and prompts to pick.

  discord list [accountname] [serverid] channels
      # lists channels for the given account & server. If multiple servers are joined,
      # we prompt to pick one, etc.

  discord set [accountname] [serverid] <keyname> <value>
      # sets a JSON key/value in bot_config for that Discord account (and optionally that server).
      # if no account/server is given, uses the "active" ones from config.

  discord chat <channel>
      # enters a REPL mode to send messages to <channel> as the active Discord account
      # (type '/quit' to exit)
"#
        .to_string()
}

/// Sets the “discord_active_account” key in bot_config.
async fn set_active_discord_account(bot_api: &Arc<dyn BotApi>, username: &str) -> String {
    let res = bot_api.set_bot_config_value("discord_active_account", username).await;
    match res {
        Ok(_) => format!("Discord active account set to '{username}'"),
        Err(e) => format!("Error setting discord_active_account => {e:?}"),
    }
}

/// Sets or prompts to select the active server for the *current* Discord account.
/// We store the chosen server ID in the JSON metadata for `(config_key="discord", config_value=<acctName>)`,
/// e.g. { "active_server": "<serverid>", "otherKeys": "..." }.
async fn set_or_select_discord_server(bot_api: &Arc<dyn BotApi>, maybe_server: Option<String>) -> String {
    // 1) read the “discord_active_account”
    let acct_opt = match bot_api.get_bot_config_value("discord_active_account").await {
        Ok(Some(a)) => a,
        _ => return "No 'discord_active_account' is set in bot_config. Please do 'discord active user <name>' first.".to_string(),
    };

    // 2) If user provided a server ID => store it
    if let Some(server_id) = maybe_server {
        match update_metadata_for_server(bot_api, &acct_opt, &server_id).await {
            Ok(_) => return format!("Active server for Discord account '{acct_opt}' set to {server_id}"),
            Err(e) => return format!("Error updating server metadata => {e:?}"),
        }
    }

    // 3) If not provided => list servers & prompt user
    // In a real implementation, you'd query the Discord API for joined servers.
    // We'll mock a few server IDs:
    let joined_servers = vec!["987654321", "2222222222", "3333333333"];
    let mut msg = String::new();
    msg.push_str("Joined servers for this account:\n");
    for (i, srv) in joined_servers.iter().enumerate() {
        msg.push_str(&format!("  [{}] {}\n", i + 1, srv));
    }
    msg.push_str("Pick a number to set the active server: ");
    println!("{}", msg);

    // read from stdin
    let mut input_line = String::new();
    let mut stdin = BufReader::new(io::stdin());
    if let Err(_) = stdin.read_line(&mut input_line).await {
        return "(Error reading input from stdin)".to_string();
    }
    let trimmed = input_line.trim();
    if trimmed.is_empty() {
        return "Cancelled; no input.".to_string();
    }
    let choice = match trimmed.parse::<usize>() {
        Ok(n) => n,
        Err(_) => 0,
    };
    if choice < 1 || choice > joined_servers.len() {
        return "Invalid selection.".to_string();
    }
    let selected_srv = joined_servers[choice - 1];

    // 4) update metadata
    match update_metadata_for_server(bot_api, &acct_opt, selected_srv).await {
        Ok(_) => format!("Active server for Discord account '{acct_opt}' set to {}", selected_srv),
        Err(e) => format!("Error updating server => {e:?}"),
    }
}

/// Helper: read the existing JSON meta for (discord, accountName). Then set "active_server": serverId.
async fn update_metadata_for_server(
    bot_api: &Arc<dyn BotApi>,
    account_name: &str,
    server_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1) read existing
    let existing_opt = bot_api.get_config_kv_meta("discord", account_name).await?;
    let mut new_meta = if let Some((_val, Some(json_data))) = existing_opt {
        json_data
    } else {
        // if no row or no meta, start a new JSON
        json!({})
    };

    // 2) set "active_server" in the JSON
    if let Some(obj) = new_meta.as_object_mut() {
        obj.insert("active_server".to_string(), json!(server_id));
    }

    // 3) store it
    bot_api.set_config_kv_meta("discord", account_name, Some(new_meta)).await?;
    Ok(())
}

/// Lists all channels for the given account + server.
/// Syntax: "discord list [acct] [server] channels"
async fn list_discord_channels(bot_api: &Arc<dyn BotApi>, subargs: &[&str]) -> String {
    // subargs might be: ["list", "myBot", "99999", "channels"]
    // or just ["list", "channels"]
    // We'll parse to see if they gave [acct], [server], or not.
    let mut account_name: Option<&str> = None;
    let mut server_id: Option<&str> = None;

    // ignore the final "channels"
    let relevant = &subargs[0..subargs.len()-1]; // drop the last "channels"

    match relevant.len() {
        0 => {}, // no arguments => we'll attempt to get from config
        1 => {
            account_name = Some(relevant[0]);
        }
        2 => {
            // e.g. ["myBot", "99999"]
            account_name = Some(relevant[0]);
            server_id    = Some(relevant[1]);
        }
        _ => {}
    }

    // If not provided, fall back to "discord_active_account"
    let acct = match account_name {
        Some(a) => a.to_string(),
        None => match bot_api.get_bot_config_value("discord_active_account").await {
            Ok(Some(val)) => val,
            _ => return "No 'discord_active_account' is set. Provide an account name or set one.".to_string(),
        }
    };

    // If server not provided, read from the metadata
    let srv = match server_id {
        Some(s) => s.to_string(),
        None => {
            let existing_opt = match bot_api.get_config_kv_meta("discord", &acct).await {
                Ok(val) => val,
                Err(e) => return format!("Error reading config meta for 'discord'/'{acct}' => {e:?}"),
            };
            if let Some((_val, Some(json_data))) = existing_opt {
                if let Some(active_srv) = json_data.get("active_server").and_then(|v| v.as_str()) {
                    active_srv.to_string()
                } else {
                    // no active_server found
                    return format!("No 'active_server' set for account '{acct}'. Use 'discord active server' first or specify a server ID.");
                }
            } else {
                return format!("No metadata found for 'discord'/'{acct}'. Use 'discord active server' first or specify a server ID.");
            }
        }
    };

    // In a real bot, you'd query Discord's HTTP API for the channels of that server.
    // We'll just mock it:
    let dummy_channels = vec!["general", "bot", "music-voice", "random"];
    let mut output = format!("Channels for Discord account='{}' server='{}':\n", acct, srv);
    for (i, ch) in dummy_channels.iter().enumerate() {
        output.push_str(&format!("  [{}] {}\n", i+1, ch));
    }
    output
}

/// Sets a key/value in the JSON metadata for a Discord account, possibly also referencing a server.
async fn set_discord_metadata(bot_api: &Arc<dyn BotApi>, args: &[&str]) -> String {
    //
    // Usage: discord set [accountname] [servername] <keyname> <value...>
    //
    // If accountname and servername are omitted, we use the “discord_active_account”
    // and that account’s "active_server". If only one is given, we treat it as account name.
    //
    // Example:
    //   discord set announcements #general
    //   discord set myAcct 12345 announcements #general
    //
    // Implementation approach:
    //   1. parse out optional account/server
    //   2. parse key + value
    //   3. read existing JSON from get_config_kv_meta("discord", <acct>)
    //   4. update either top-level <keyname> or nested per-server structure
    //   5. set_config_kv_meta
    //

    let mut idx_for_key = 1; // index in args where <keyname> should start
    let mut account_name: Option<String> = None;
    let mut server_id: Option<String> = None;

    // We have at least 3 tokens after "discord set", so minimum length is 3 => which might be just <key> <value>
    // but user might supply account + server first.
    // We will do a small parse:

    // Strategy: if the second token does NOT look like a “keyname”, treat it as account name or server name.
    // A valid <keyname> cannot have spaces, but let's be simpler: we will do:
    // - If the second token cannot possibly be an account or server, we might guess it's a key.
    // - We'll be more straightforward: We’ll check how many total tokens we have left.

    // "discord set" is the first two tokens => so the sub-slice is args[1..].
    // That sub-slice has length = (args.len() - 1).

    // We want to identify <keyname> as the second-to-last item or the next one if we have optional account/server.
    // Example:
    //   "discord set key val" => sub-slice is [ "key", "val" ] => key= "key", val= "val"
    //   "discord set myAcct key val" => sub-slice is [ "myAcct", "key", "val" ] => acct= "myAcct", key= "key", val= "val"
    //   "discord set myAcct 12345 key val" => sub-slice is [ "myAcct", "12345", "key", "val" ] => acct= "myAcct", server= "12345", key= "key", val= "val"

    let sub = &args[1..]; // skip the "set" token
    if sub.len() < 2 {
        return "Usage: discord set [account] [server] <key> <value...>".to_string();
    }

    // we’ll find the index where key starts
    // if sub.len()==2 => ( key, val ) => no account/server
    // if sub.len()==3 => could be ( acct, key, val ) or ( key, val, ??? ) but the latter is invalid
    // if sub.len()==4 => could be ( acct, server, key, val )

    if sub.len() == 2 {
        // interpret sub[0] as key, sub[1] as value
        idx_for_key = 0;
    } else if sub.len() == 3 {
        // interpret sub[0] as account, sub[1] as key, sub[2..] as value
        account_name = Some(sub[0].to_string());
        idx_for_key = 1;
    } else {
        // sub.len() >= 4 => interpret sub[0] as account, sub[1] as server, sub[2] as key, sub[3..] as value...
        account_name = Some(sub[0].to_string());
        server_id    = Some(sub[1].to_string());
        idx_for_key = 2;
    }

    let keyname = sub[idx_for_key];
    let val_slice = &sub[(idx_for_key + 1)..];
    if val_slice.is_empty() {
        return format!("No value given after key='{keyname}'. Usage: discord set [acct] [srv] <key> <value...>");
    }
    let joined_value = val_slice.join(" ");

    // if account_name is not specified, we read from "discord_active_account"
    let acct = match account_name {
        Some(a) => a,
        None => match bot_api.get_bot_config_value("discord_active_account").await {
            Ok(Some(val)) => val,
            _ => {
                return "No 'discord_active_account' is set. Provide an account or set one first.".to_string();
            }
        }
    };

    // if server_id is not specified, read from that account’s metadata
    let srv_opt = if let Some(s) = server_id {
        Some(s)
    } else {
        let existing_opt = bot_api.get_config_kv_meta("discord", &acct).await;
        match existing_opt {
            Ok(Some((_val, Some(json_data)))) => {
                if let Some(active_srv) = json_data.get("active_server").and_then(|v| v.as_str()) {
                    Some(active_srv.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    };

    // Now we read the existing metadata for (config_key="discord", config_value=<acct>)
    let existing_opt = match bot_api.get_config_kv_meta("discord", &acct).await {
        Ok(e) => e,
        Err(e) => return format!("Error reading config meta for 'discord'/'{acct}' => {e:?}"),
    };
    let mut meta = if let Some((_val, Some(json_data))) = existing_opt {
        json_data
    } else {
        json!({})
    };

    // We decide whether to store key at top-level in meta or inside a server sub-object
    if let Some(server_id) = srv_opt {
        // let's keep a sub-object "servers" in the JSON if we want to store per-server data
        // or we can place it top-level. The instructions were not super specific,
        // so let's do:  meta["server_data"][server_id][keyname] = joined_value
        let obj = meta.as_object_mut().unwrap();
        let servers = obj.entry("server_data").or_insert_with(|| json!({}));
        if let Some(servers_obj) = servers.as_object_mut() {
            let server_obj = servers_obj.entry(server_id.clone()).or_insert_with(|| json!({}));
            if let Some(srv_map) = server_obj.as_object_mut() {
                srv_map.insert(keyname.to_string(), json!(joined_value));
            }
        }
    } else {
        // if no server is in use, store top-level
        if let Some(obj) = meta.as_object_mut() {
            obj.insert(keyname.to_string(), json!(joined_value));
        }
    }

    // store updated meta
    match bot_api.set_config_kv_meta("discord", &acct, Some(meta)).await {
        Ok(_) => format!("Set Discord metadata for account='{acct}' key='{keyname}' => '{joined_value}'"),
        Err(e) => format!("Error updating discord config => {e:?}"),
    }
}

/// Opens a mini REPL to send messages to the given Discord channel.
/// Uses `/quit` to exit. In a real implementation, you would call a specialized
/// Discord API method or platform_manager function to actually send messages.
async fn open_discord_chat_mode(bot_api: &Arc<dyn BotApi>, channel: &str) -> String {
    println!("(Discord chat) Type '/quit' to exit. Sending to channel '{channel}'...");
    let mut stdin = BufReader::new(io::stdin());

    loop {
        let mut line = String::new();
        if stdin.read_line(&mut line).await.is_err() {
            return "(Error reading from stdin)".to_string();
        }
        let text = line.trim();
        if text.eq_ignore_ascii_case("/quit") {
            return "Exiting Discord chat mode.".to_string();
        }

        // In a real bot, we would do:
        //   bot_api.send_discord_message(activeDiscordAccount, channel, text).await
        // For now, we just print:
        println!("(You => {channel}): {text}");
    }
}
