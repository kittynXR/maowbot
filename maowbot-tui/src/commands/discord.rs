// ========================================================
// File: maowbot-tui/src/commands/discord.rs
// ========================================================
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
        // Live role management for Twitch streamers
        // ------------------------------------------------------------------
        "liverole" => {
            if args.len() < 2 {
                return "Usage: discord liverole <guildId> <roleId> OR discord liverole list OR discord liverole remove <guildId>".to_string();
            }
            
            match args[1].to_lowercase().as_str() {
                "list" => {
                    match bot_api.list_discord_live_roles().await {
                        Ok(live_roles) => {
                            if live_roles.is_empty() {
                                "No live roles configured.".to_string()
                            } else {
                                let mut out = String::from("Discord live roles (assigned to users streaming on Twitch):\n");
                                for role in live_roles {
                                    out.push_str(&format!(" - Guild: {}, Role: {}\n", role.guild_id, role.role_id));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing live roles: {e}"),
                    }
                }
                "remove" => {
                    if args.len() < 3 {
                        return "Usage: discord liverole remove <guildId>".to_string();
                    }
                    let guild_id = args[2];
                    match bot_api.delete_discord_live_role(guild_id).await {
                        Ok(_) => format!("Removed live role configuration for guild {}", guild_id),
                        Err(e) => format!("Error removing live role: {e}"),
                    }
                }
                _ => {
                    // Handle "discord liverole <guildId> <roleId>"
                    if args.len() < 3 {
                        return "Usage: discord liverole <guildId> <roleId>".to_string();
                    }
                    let guild_id = args[1];
                    let role_id = args[2];
                    match bot_api.set_discord_live_role(guild_id, role_id).await {
                        Ok(_) => format!("Set live role {} for guild {}", role_id, guild_id),
                        Err(e) => format!("Error setting live role: {e}"),
                    }
                }
            }
        }
        
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
        // 3) discord event subcommands (including addrole/delrole)
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

            match bot_api.send_discord_message("vrcgirl", server_id, channel_id, &text).await {
                Ok(_) => format!("Sent message to channel {channel_id}: '{text}'"),
                Err(e) => format!("Error sending Discord message => {e}"),
            }
        }

        // ------------------------------------------------------------------
        // 5) discord roles [guildId]
        // ------------------------------------------------------------------
        "roles" => {
            // Determine which Discord credential to use
            let all_discord_creds = match bot_api.list_credentials(Some(Platform::Discord)).await {
                Ok(creds) => creds,
                Err(e) => return format!("Error listing Discord credentials: {e}"),
            };
            if all_discord_creds.is_empty() {
                return "No Discord credentials found.".to_string();
            }
            let chosen_account_name = if all_discord_creds.len() == 1 {
                all_discord_creds[0].user_name.clone()
            } else {
                return "Multiple Discord accounts found; please specify one first, e.g. 'discord guilds <acct>'.".to_string();
            };

            let guild_id = if args.len() > 1 {
                args[1].to_string()
            } else {
                match bot_api.list_discord_guilds(&chosen_account_name).await {
                    Ok(guilds) => {
                        if guilds.len() == 1 {
                            guilds[0].guild_id.clone()
                        } else if guilds.is_empty() {
                            return "No guilds found for that account. Provide a guild ID explicitly.".to_string();
                        } else {
                            return "Multiple guilds found; please specify guild ID explicitly.".to_string();
                        }
                    }
                    Err(e) => return format!("Error listing guilds => {e}"),
                }
            };
            match bot_api.list_discord_roles(&chosen_account_name, &guild_id).await {
                Ok(roles) => {
                    if roles.is_empty() {
                        format!("No roles found for guild ID '{}'.", guild_id)
                    } else {
                        let mut out = format!("Roles for guild ID '{}':\n", guild_id);
                        for (role_id, role_name) in roles {
                            out.push_str(&format!(" - {}: {}\n", role_id, role_name));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing roles: {e}"),
            }
        }

        _ => show_usage(),
    }
}

/// --------------------------------------------------------------------------
/// Helper for “discord event …” subcommands
/// Supports:
///   discord event list
///   discord event add <eventname> <channelid> [guildid] [acctOrCred]
///   discord event remove <eventname> <channelid> [guildid] [acctOrCred]
///   discord event addrole <eventname> <roleid>
///   discord event delrole <eventname> <roleid>
/// --------------------------------------------------------------------------
async fn handle_discord_event_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: discord event (list|add|remove|addrole|delrole) ...".to_string();
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
                            let roles_str = if let Some(roles) = rec.ping_roles {
                                format!("ping_roles={:?}", roles)
                            } else {
                                "ping_roles=None".to_string()
                            };
                            out.push_str(&format!(
                                " - event='{}' guild='{}' channel='{}' {} {}\n",
                                rec.event_name, rec.guild_id, rec.channel_id, cred_str, roles_str
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing event configs: {e}"),
            }
        }
        "add" => {
            // New format: discord event add <eventname> <channelid> [guildid] [acctOrCred]
            if args.len() < 3 {
                return "Usage: discord event add <eventname> <channelid> [guildid] [acctOrCred]".to_string();
            }
            let event_name = args[1];
            let channel_arg = args[2];
            let maybe_guild = if args.len() >= 4 { Some(args[3]) } else { None };
            let maybe_acct_or_cred = if args.len() >= 5 { Some(args[4]) } else { None };

            // Determine guild string
            let guild_str = if let Some(g) = maybe_guild {
                g.to_string()
            } else {
                // If no guild is specified, try to use the single guild from the default account.
                let accounts = match bot_api.list_credentials(Some(Platform::Discord)).await {
                    Ok(list) => list,
                    Err(e) => return format!("Could not list Discord credentials: {e}"),
                };
                if accounts.is_empty() {
                    return "No Discord accounts found in credentials.".to_string();
                }
                let single_account_name = if accounts.len() == 1 {
                    accounts[0].user_name.clone()
                } else {
                    return "Multiple Discord accounts found; please specify guild ID explicitly.".to_string();
                };
                match bot_api.list_discord_guilds(&single_account_name).await {
                    Ok(guilds) => {
                        if guilds.len() == 1 {
                            guilds[0].guild_id.clone()
                        } else if guilds.is_empty() {
                            return "No guilds found for that account. Provide a guild ID explicitly.".to_string();
                        } else {
                            return "Multiple guilds found; please specify guild ID explicitly.".to_string();
                        }
                    }
                    Err(e) => return format!("Error listing guilds => {e}"),
                }
            };

            // Parse credential (optional)
            let mut respond_with_cred: Option<Uuid> = None;
            if let Some(acct_or_cred) = maybe_acct_or_cred {
                if let Ok(cid) = Uuid::parse_str(acct_or_cred) {
                    respond_with_cred = Some(cid);
                } else {
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

            // Now call the API
            match bot_api
                .add_discord_event_config(&event_name, &guild_str, &channel_arg, respond_with_cred)
                .await
            {
                Ok(_) => format!(
                    "Added event config: event='{}' channel='{}' guild='{}' cred={:?}",
                    event_name, channel_arg, guild_str, respond_with_cred
                ),
                Err(e) => format!("Error adding event config => {e}"),
            }
        }
        "remove" => {
            // New format: discord event remove <eventname> <channelid> [guildid] [acctOrCred]
            if args.len() < 3 {
                return "Usage: discord event remove <eventname> <channelid> [guildid] [acctOrCred]".to_string();
            }
            let event_name = args[1];
            let channel_arg = args[2];
            let maybe_guild = if args.len() >= 4 { Some(args[3]) } else { None };
            let maybe_acct_or_cred = if args.len() >= 5 { Some(args[4]) } else { None };

            let guild_str = if let Some(g) = maybe_guild {
                g.to_string()
            } else {
                // Use a default account's single guild if available
                let single_account_name = "vrcgirl";
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

            let mut respond_with_cred: Option<Uuid> = None;
            if let Some(acct_or_cred) = maybe_acct_or_cred {
                if let Ok(cid) = Uuid::parse_str(acct_or_cred) {
                    respond_with_cred = Some(cid);
                } else {
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
                .remove_discord_event_config(&event_name, &guild_str, &channel_arg, respond_with_cred)
                .await
            {
                Ok(_) => format!(
                    "Removed event config for event='{}' channel='{}' guild='{}' cred={:?}",
                    event_name, channel_arg, guild_str, respond_with_cred
                ),
                Err(e) => format!("Error removing event config => {e}"),
            }
        }
        "addrole" => {
            // New command: discord event addrole <eventname> <roleid>
            if args.len() < 3 {
                return "Usage: discord event addrole <eventname> <roleid>".to_string();
            }
            let event_name = args[1];
            let role_id = args[2];
            match bot_api.add_discord_event_role(event_name, role_id).await {
                Ok(_) => format!("Added role {} to event '{}'.", role_id, event_name),
                Err(e) => format!("Error adding role: {e}"),
            }
        }
        "delrole" => {
            // New command: discord event delrole <eventname> <roleid>
            if args.len() < 3 {
                return "Usage: discord event delrole <eventname> <roleid>".to_string();
            }
            let event_name = args[1];
            let role_id = args[2];
            match bot_api.remove_discord_event_role(event_name, role_id).await {
                Ok(_) => format!("Removed role {} from event '{}'.", role_id, event_name),
                Err(e) => format!("Error removing role: {e}"),
            }
        }
        _ => "Usage: discord event (list|add|remove|addrole|delrole) ...".to_string(),
    }
}

fn show_usage() -> String {
    r#"Discord Commands:
  discord guilds [accountNameOrUUID]
      -> list all guilds for that Discord account
  discord channels [guildId]
      -> list channels in the single known guild or the specified one
  discord event list
  discord event add <eventName> <channelId> [guildId] [acctOrCred]
  discord event remove <eventName> <channelId> [guildId] [acctOrCred]
  discord event addrole <eventName> <roleid>
  discord event delrole <eventName> <roleid>
  discord msg <serverId> <channelId> [message text...]
  discord roles [guildId]
      -> list all role IDs and names for the specified guild (or the single guild if only one is joined)
  discord liverole <guildId> <roleId>
      -> set role to assign to users when they're streaming on Twitch
  discord liverole list
      -> list currently configured live roles
  discord liverole remove <guildId>
      -> remove live role configuration for the specified guild
"#
        .to_string()
}
