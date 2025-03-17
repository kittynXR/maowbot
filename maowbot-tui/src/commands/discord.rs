use std::sync::Arc;
use uuid::Uuid;
use maowbot_common::models::platform::Platform;
use maowbot_common::traits::api::BotApi;

/// Handle "discord" TUI commands.

pub async fn handle_discord_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return show_usage();
    }

    match args[0].to_lowercase().as_str() {
        // ------------------------------------------------------------------
        // 1) discord guilds [accountNameOrUUID]
        // ------------------------------------------------------------------
        "guilds" => {
            // If there's an argument after "guilds", treat it as either an accountName or a credential UUID.
            let maybe_acct_str = if args.len() > 1 {
                Some(args[1])
            } else {
                None
            };

            // 1) Gather all Discord credentials from the database
            let all_discord_creds = match bot_api.list_credentials(Some(Platform::Discord)).await {
                Ok(creds) => creds,
                Err(e) => return format!("Error listing Discord credentials: {e}"),
            };
            if all_discord_creds.is_empty() {
                return "No Discord credentials found.".to_string();
            }

            // 2) If exactly one cred, we can use it, unless the user tries to specify another
            let chosen_account_name = if let Some(acct_str) = maybe_acct_str {
                // Attempt to parse as UUID or find a matching user_name
                if let Ok(parsed_uuid) = Uuid::parse_str(acct_str) {
                    // See if we have a credential with that ID
                    if let Some(c) = all_discord_creds.iter().find(|c| c.credential_id == parsed_uuid) {
                        c.user_name.clone()
                    } else {
                        return format!("No Discord credential found with ID={parsed_uuid}");
                    }
                } else {
                    // treat it as a user_name
                    if let Some(c) = all_discord_creds.iter().find(|c| c.user_name == acct_str) {
                        c.user_name.clone()
                    } else {
                        return format!("No Discord credential found with accountName='{acct_str}'");
                    }
                }
            } else if all_discord_creds.len() == 1 {
                // exactly one => no argument required
                all_discord_creds[0].user_name.clone()
            } else {
                // multiple creds => user must specify
                return "Multiple Discord credentials found; please specify which one: discord guilds <accountNameOrUUID>".to_string();
            };

            // 3) Now list the guilds
            match bot_api.list_discord_guilds(&chosen_account_name).await {
                Ok(guilds) => {
                    if guilds.is_empty() {
                        format!("No guilds found for Discord account '{chosen_account_name}'.")
                    } else {
                        let mut out = format!("Discord guilds for account='{chosen_account_name}':\n");
                        for g in guilds {
                            out.push_str(&format!(" - {} (ID={})\n", g.guild_name, g.guild_id));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing guilds => {e}"),
            }
        }

        // ------------------------------------------------------------------
        // 2) discord channels [guildId]
        // ------------------------------------------------------------------
        "channels" => {
            // We might have an optional guildId
            let maybe_guild_id = if args.len() > 1 {
                Some(args[1])
            } else {
                None
            };

            // 1) figure out which Discord credential to use
            let all_discord_creds = match bot_api.list_credentials(Some(Platform::Discord)).await {
                Ok(creds) => creds,
                Err(e) => return format!("Error listing Discord credentials: {e}"),
            };
            if all_discord_creds.is_empty() {
                return "No Discord credentials found for Discord.".to_string();
            }
            let chosen_account_name = if all_discord_creds.len() == 1 {
                all_discord_creds[0].user_name.clone()
            } else {
                // multiple -> user must specify one
                return "Multiple Discord accounts found; please specify one first, e.g. 'discord guilds <acct>'.".to_string();
            };

            // 2) List all guilds for that account to see if there's exactly one guild
            let guilds = match bot_api.list_discord_guilds(&chosen_account_name).await {
                Ok(g) => g,
                Err(e) => return format!("Error listing guilds => {e}"),
            };
            if guilds.is_empty() {
                return format!("No guilds found for account='{chosen_account_name}'.");
            }

            let final_guild_id = if let Some(g) = maybe_guild_id {
                g.to_string()
            } else {
                if guilds.len() == 1 {
                    // Use the single guild
                    guilds[0].guild_id.clone()
                } else {
                    return "Multiple guilds found; specify a guild ID: discord channels <guildId>".to_string();
                }
            };

            // 3) Now list the channels
            match bot_api.list_discord_channels(&chosen_account_name, &final_guild_id).await {
                Ok(channels) => {
                    if channels.is_empty() {
                        format!("No channels found in guild={final_guild_id} for account='{chosen_account_name}'.")
                    } else {
                        let mut out = format!(
                            "Discord channels for account='{chosen_account_name}', guild='{final_guild_id}':\n"
                        );
                        for ch in channels {
                            out.push_str(&format!(" - {} (ID={})\n", ch.channel_name, ch.channel_id));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing channels => {e}"),
            }
        }

        // ------------------------------------------------------------------
        // 3) discord event ...
        // ------------------------------------------------------------------
        "event" => handle_discord_event_command(&args[1..], bot_api).await,

        // ------------------------------------------------------------------
        // 4) discord msg <serverId> <channelId> <message...>
        // ------------------------------------------------------------------
        "msg" => {
            if args.len() < 3 {
                return "Usage: discord msg <serverId> <channelId> <message...>".to_string();
            }
            let server_id = args[1];
            let channel_id = args[2];
            let text = if args.len() > 3 {
                args[3..].join(" ")
            } else {
                "".to_string()
            };

            match bot_api.send_discord_message("cutecat_chat", server_id, channel_id, &text).await {
                Ok(_) => format!("Sent message to channel {channel_id}: '{text}'"),
                Err(e) => format!("Error sending Discord message => {e}"),
            }
        }

        _ => show_usage(),
    }
}

/// --------------------------------------------------------------------------
/// Helper for “discord event add|remove|list” subcommands
/// Usage:
///   discord event list
///   discord event add <eventname> [guildid] <channelid> [accountOrCredUUID]
///   discord event remove <eventname> [guildid] <channelid> [accountOrCredUUID]
/// --------------------------------------------------------------------------
async fn handle_discord_event_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: discord event (list|add|remove) ...".to_string();
    }

    match args[0].to_lowercase().as_str() {
        "list" => {
            match bot_api.list_discord_event_configs().await {
                Ok(evconfigs) => {
                    if evconfigs.is_empty() {
                        "No Discord event configs found.".to_string()
                    } else {
                        let mut out = String::from("Discord event configs:\n");
                        for rec in evconfigs {
                            let cred_str = if let Some(cid) = rec.respond_with_credential {
                                format!("credential={cid}")
                            } else {
                                "credential=None".to_string()
                            };
                            out.push_str(&format!(
                                " - event='{}' guild='{}' channel='{}' {}\n",
                                rec.event_name, rec.guild_id, rec.channel_id, cred_str
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing event configs: {e}"),
            }
        }
        "add" => {
            if args.len() < 2 {
                return "Usage: discord event add <eventname> [guildid] <channelid> [acctOrCred]".to_string();
            }
            let mut idx = 1;
            let event_name = args[idx];
            idx += 1;

            // We may or may not have a "guildid" next
            // We also need a channelId. Let's parse them carefully.
            // We'll see how many remaining arguments we have.
            if args.len() < (idx + 1) {
                return "Usage: discord event add <eventname> [guildid] <channelid> [acctOrCred]".to_string();
            }

            // We'll store optional guild first
            // Then mandatory channel
            let mut maybe_guild = None;
            let mut channel_arg = "";

            // We'll try to guess if the next arg is a channel or a guild.
            // If there's only one guild in the system, we can skip asking for it.
            let accounts = match bot_api.list_credentials(Some(Platform::Discord)).await {
                Ok(list) => list,
                Err(e) => return format!("Could not list Discord credentials: {e}"),
            };

            // We'll see if the next argument is a numeric or large Snowflake => let's assume it might be guildID
            // But user might skip guild if there's only one. Let's see how many guilds we have for the single
            // or active account. If there's multiple accounts or multiple guilds, we must parse or fail.
            // For brevity, we do a minimal approach:

            // Figure out how many Discord accounts we have:
            let discord_accounts: Vec<_> = accounts.into_iter().collect();
            let single_account_name = if discord_accounts.len() == 1 {
                discord_accounts[0].user_name.clone()
            } else if discord_accounts.is_empty() {
                return "No Discord accounts found in credentials.".to_string();
            } else {
                // multiple accounts
                // The user might specify the last argument as an account name or credential UUID
                // We'll just do partial checking: let them pass it explicitly.
                // We'll keep going but won't handle multiple seamlessly.
                // We'll store that for the respond_with_credential if given.
                discord_accounts[0].user_name.clone()
            };

            // We'll next see if we can parse a possible guild ID from the next arg
            let next_arg = args[idx];
            if is_possible_snowflake(next_arg) && args.len() > (idx + 1) {
                // We'll assume it's guild
                maybe_guild = Some(next_arg);
                idx += 1;
                // now the next arg is channel
                if args.len() <= idx {
                    return "Expected a <channelid> after the guildid.".to_string();
                }
                channel_arg = args[idx];
                idx += 1;
            } else {
                // We skip guild, let's see if there's only one guild
                channel_arg = next_arg;
                idx += 1;
            }

            // Finally, optional accountOrCred
            let maybe_acct_or_cred = if args.len() > idx {
                Some(args[idx])
            } else {
                None
            };

            // If we did not specify guild, see if there's exactly one known guild for the default account
            let guild_str = if let Some(g) = maybe_guild {
                g.to_string()
            } else {
                // Try listing guilds for single_account_name
                match bot_api.list_discord_guilds(&single_account_name).await {
                    Ok(guilds) => {
                        if guilds.len() == 1 {
                            guilds[0].guild_id.clone()
                        } else if guilds.is_empty() {
                            return "No guilds found in memory for that account. Provide a guild ID explicitly.".to_string();
                        } else {
                            return "Multiple guilds found; please specify [guildid].".to_string();
                        }
                    }
                    Err(e) => return format!("Error listing guilds => {e}"),
                }
            };

            // parse channel
            let channel_str = channel_arg.to_string();

            // parse credential (accountName or credential UUID)
            let mut respond_with_cred: Option<Uuid> = None;
            if let Some(acct_or_cred) = maybe_acct_or_cred {
                // check if it might be a UUID
                if let Ok(cid) = Uuid::parse_str(acct_or_cred) {
                    respond_with_cred = Some(cid);
                } else {
                    // or see if it matches a known credential name:
                    let platform_creds = match bot_api.list_credentials(Some(Platform::Discord)).await {
                        Ok(c) => c,
                        Err(e) => return format!("Could not fetch credentials: {e}"),
                    };
                    let found = platform_creds.iter().find(|c| c.user_name == acct_or_cred);
                    if let Some(fc) = found {
                        respond_with_cred = Some(fc.credential_id);
                    } else {
                        return format!("Could not find a Discord credential for '{acct_or_cred}'.");
                    }
                }
            } else {
                // if we have multiple accounts, user must specify
                // we'll just do nothing if there's exactly one
            }

            // Now call the API
            match bot_api
                .add_discord_event_config(&event_name, &guild_str, &channel_str, respond_with_cred)
                .await
            {
                Ok(_) => format!(
                    "Added event config: event='{}' guild='{}' channel='{}' cred={:?}",
                    event_name, guild_str, channel_str, respond_with_cred
                ),
                Err(e) => format!("Error adding event config => {e}"),
            }
        }

        "remove" => {
            // Usage: discord event remove <eventname> [guildid] <channelid> [accountOrCred]
            if args.len() < 2 {
                return "Usage: discord event remove <eventname> [guildid] <channelid> [acctOrCred]".to_string();
            }

            let mut idx = 1;
            let event_name = args[idx];
            idx += 1;

            let next_arg = args[idx];
            idx += 1;

            let mut maybe_guild = None;
            let mut channel_arg = "";

            if is_possible_snowflake(next_arg) && args.len() > idx {
                maybe_guild = Some(next_arg);
                channel_arg = args[idx];
                idx += 1;
            } else {
                channel_arg = next_arg;
            }

            let maybe_acct_or_cred = if args.len() > idx {
                Some(args[idx])
            } else {
                None
            };

            let single_account_name = "cutecat_chat"; // for brevity, we re-use
            let guild_str = if let Some(g) = maybe_guild {
                g.to_string()
            } else {
                // attempt to see if there's only one guild:
                match bot_api.list_discord_guilds(single_account_name).await {
                    Ok(guilds) => {
                        if guilds.len() == 1 {
                            guilds[0].guild_id.clone()
                        } else if guilds.is_empty() {
                            return "No guilds found. Please specify a guild ID.".to_string();
                        } else {
                            return "Multiple guilds found; specify the guild ID.".to_string();
                        }
                    }
                    Err(e) => return format!("Error listing guilds => {e}"),
                }
            };

            let channel_str = channel_arg.to_string();

            let mut respond_with_cred: Option<Uuid> = None;
            if let Some(acct_or_cred) = maybe_acct_or_cred {
                if let Ok(cid) = Uuid::parse_str(acct_or_cred) {
                    respond_with_cred = Some(cid);
                } else {
                    // see if it’s a known user_name for a Discord credential
                    let platform_creds = match bot_api.list_credentials(Some(Platform::Discord)).await {
                        Ok(c) => c,
                        Err(e) => return format!("Could not fetch credentials: {e}"),
                    };
                    let found = platform_creds.iter().find(|c| c.user_name == acct_or_cred);
                    if let Some(fc) = found {
                        respond_with_cred = Some(fc.credential_id);
                    } else {
                        return format!("Could not find a Discord credential for '{acct_or_cred}'.");
                    }
                }
            }

            match bot_api
                .remove_discord_event_config(&event_name, &guild_str, &channel_str, respond_with_cred)
                .await
            {
                Ok(_) => format!(
                    "Removed event config for event='{}' guild='{}' channel='{}' cred={:?}",
                    event_name, guild_str, channel_str, respond_with_cred
                ),
                Err(e) => format!("Error removing event config => {e}"),
            }
        }

        _ => "Usage: discord event (list|add|remove) ...".to_string(),
    }
}

fn show_usage() -> String {
    r#"Discord Commands:
  discord guilds [accountNameOrUUID]
      -> list all guilds for that Discord account
  discord channels [guildId]
      -> list channels in the single known guild or the specified one
  discord event list
  discord event add <eventName> [guildId] <channelId> [accountOrCredUUID]
  discord event remove <eventName> [guildId] <channelId> [accountOrCredUUID]
  discord msg <serverId> <channelId> [message text...]
"#
        .to_string()
}

/// A small helper to check if a string might be a Discord snowflake (18-20 digits).
fn is_possible_snowflake(s: &str) -> bool {
    if s.len() < 5 || s.len() > 20 {
        return false;
    }
    s.chars().all(|c| c.is_ascii_digit())
}
