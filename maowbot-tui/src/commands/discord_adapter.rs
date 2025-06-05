// Discord command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::discord::DiscordCommands};
use maowbot_proto::maowbot::services::{ListActiveRuntimesRequest, ListCredentialsRequest};
use maowbot_proto::maowbot::common::Platform;

/// Get the connected Discord account name
async fn get_connected_discord_account(client: &GrpcClient) -> Result<String, String> {
    // First check if we have any active Discord runtimes
    let request = ListActiveRuntimesRequest {
        platforms: vec![Platform::Discord as i32],
    };
    
    let response = client.platform.clone()
        .list_active_runtimes(request)
        .await
        .map_err(|e| format!("Failed to list active runtimes: {}", e))?;
    
    let runtimes = response.into_inner().runtimes;
    
    if runtimes.is_empty() {
        return Err("No Discord connections active. Use 'connection start discord' first.".to_string());
    }
    
    if runtimes.len() == 1 {
        Ok(runtimes[0].account_name.clone())
    } else {
        // Multiple accounts connected - return the first one for now
        // In the future, we could prompt the user to select
        Ok(runtimes[0].account_name.clone())
    }
}

/// Prompt user to select a guild from a list
async fn select_guild_interactive(
    client: &GrpcClient,
    account_name: &str,
    purpose: &str,
) -> Result<String, String> {
    // List guilds for the account
    match DiscordCommands::list_guilds(client, account_name).await {
        Ok(result) => {
            let guilds = &result.data.guilds;
            
            if guilds.is_empty() {
                return Err("Bot is not in any Discord guilds.".to_string());
            }
            
            if guilds.len() == 1 {
                // Only one guild, use it automatically
                Ok(guilds[0].guild_id.clone())
            } else {
                // Multiple guilds - list them and ask user to specify
                let mut msg = format!("Multiple guilds found. Please specify guild ID for {}:\n", purpose);
                for guild in guilds {
                    msg.push_str(&format!("  - {} (ID: {})\n", guild.name, guild.guild_id));
                }
                msg.push_str("\nUsage: discord list ");
                msg.push_str(purpose);
                msg.push_str(" <guildId>");
                Err(msg)
            }
        }
        Err(e) => Err(format!("Failed to list guilds: {}", e)),
    }
}

pub async fn handle_discord_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return show_usage();
    }

    match args[0].to_lowercase().as_str() {
        "list" => {
            if args.len() < 2 {
                return "Usage: discord list (guilds|channels|roles|members|liveroles) [args...]".to_string();
            }
            
            match args[1].to_lowercase().as_str() {
                "guilds" => {
                    // Get connected Discord account
                    let account_name = match get_connected_discord_account(client).await {
                        Ok(name) => name,
                        Err(e) => return e,
                    };
                    
                    match DiscordCommands::list_guilds(client, &account_name).await {
                        Ok(result) => {
                            if result.data.guilds.is_empty() {
                                "Bot is not in any Discord guilds.".to_string()
                            } else {
                                let mut out = format!("Discord guilds ({}@discord):\n", account_name);
                                for guild in result.data.guilds {
                                    out.push_str(&format!(
                                        " - {} (ID: {}, Members: {})\n",
                                        guild.name,
                                        guild.guild_id,
                                        guild.member_count
                                    ));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing guilds: {}", e),
                    }
                }
                
                "channels" => {
                    let guild_id = if args.len() >= 3 {
                        args[2].to_string()
                    } else {
                        // Try to auto-detect guild
                        let account_name = match get_connected_discord_account(client).await {
                            Ok(name) => name,
                            Err(e) => return e,
                        };
                        
                        match select_guild_interactive(client, &account_name, "channels").await {
                            Ok(id) => id,
                            Err(e) => return e,
                        }
                    };
                    
                    let account_name = match get_connected_discord_account(client).await {
                        Ok(name) => name,
                        Err(e) => return e,
                    };
                    
                    match DiscordCommands::list_channels(client, &account_name, &guild_id).await {
                        Ok(result) => {
                            if result.data.channels.is_empty() {
                                format!("No channels found in guild {}", guild_id)
                            } else {
                                let mut out = format!("Channels in guild {}:\n", guild_id);
                                for channel in result.data.channels {
                                    out.push_str(&format!(
                                        " - {} (ID: {}, Type: {:?})\n",
                                        channel.name,
                                        channel.channel_id,
                                        channel.r#type
                                    ));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing channels: {}", e),
                    }
                }
                
                "roles" => {
                    let guild_id = if args.len() >= 3 {
                        args[2].to_string()
                    } else {
                        // Try to auto-detect guild
                        let account_name = match get_connected_discord_account(client).await {
                            Ok(name) => name,
                            Err(e) => return e,
                        };
                        
                        match select_guild_interactive(client, &account_name, "roles").await {
                            Ok(id) => id,
                            Err(e) => return e,
                        }
                    };
                    
                    let account_name = match get_connected_discord_account(client).await {
                        Ok(name) => name,
                        Err(e) => return e,
                    };
                    
                    match DiscordCommands::list_roles(client, &account_name, &guild_id).await {
                        Ok(result) => {
                            if result.data.roles.is_empty() {
                                format!("No roles found in guild {}", guild_id)
                            } else {
                                let mut out = format!("Roles in guild {}:\n", guild_id);
                                for role in result.data.roles {
                                    out.push_str(&format!(
                                        " - {} (ID: {})\n",
                                        role.name,
                                        role.role_id
                                    ));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing roles: {}", e),
                    }
                }
                
                "members" => {
                    let guild_id = if args.len() >= 3 {
                        args[2].to_string()
                    } else {
                        // Try to auto-detect guild
                        let account_name = match get_connected_discord_account(client).await {
                            Ok(name) => name,
                            Err(e) => return e,
                        };
                        
                        match select_guild_interactive(client, &account_name, "members").await {
                            Ok(id) => id,
                            Err(e) => return e,
                        }
                    };
                    
                    let account_name = match get_connected_discord_account(client).await {
                        Ok(name) => name,
                        Err(e) => return e,
                    };
                    
                    match DiscordCommands::list_members(client, &account_name, &guild_id, 50).await {
                        Ok(result) => {
                            if result.data.members.is_empty() {
                                format!("No members found in guild {}", guild_id)
                            } else {
                                let mut out = format!("Members in guild {} (showing first {}):\n", guild_id, result.data.members.len());
                                for member in &result.data.members {
                                    out.push_str(&format!(
                                        " - {} ({})\n",
                                        member.username,
                                        if member.display_name.is_empty() { "no display name" } else { &member.display_name }
                                    ));
                                }
                                if result.data.has_more {
                                    out.push_str("\n(More members available)\n");
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing members: {}", e),
                    }
                }
                
                "liveroles" => {
                    match DiscordCommands::list_live_roles(client).await {
                        Ok(result) => {
                            if result.data.live_roles.is_empty() {
                                "No live roles configured.".to_string()
                            } else {
                                let mut out = String::from("Discord live roles (assigned to users streaming on Twitch):\n");
                                for role in result.data.live_roles {
                                    out.push_str(&format!(" - Guild: {}, Role: {} ({})\n", role.guild_id, role.role_id, role.role_name));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing live roles: {}", e),
                    }
                }
                
                _ => "Unknown list subcommand. Use: discord list (guilds|channels|roles|members|liveroles)".to_string(),
            }
        }
        
        "liverole" => {
            if args.len() < 2 {
                return "Usage: discord liverole <add|remove> [args...]".to_string();
            }
            
            match args[1].to_lowercase().as_str() {
                "add" => {
                    if args.len() < 4 {
                        return "Usage: discord liverole add <guildId> <roleId>".to_string();
                    }
                    let guild_id = args[2];
                    let role_id = args[3];
                    
                    match DiscordCommands::set_live_role(client, guild_id, role_id).await {
                        Ok(_) => format!(
                            "Set live role: Guild {} will assign role {} to users who are streaming on Twitch.",
                            guild_id,
                            role_id
                        ),
                        Err(e) => format!("Error setting live role: {}", e),
                    }
                }
                "remove" => {
                    if args.len() < 3 {
                        return "Usage: discord liverole remove <guildId>".to_string();
                    }
                    let guild_id = args[2];
                    match DiscordCommands::delete_live_role(client, guild_id).await {
                        Ok(_) => format!("Removed live role configuration for guild {}", guild_id),
                        Err(e) => format!("Error removing live role: {}", e),
                    }
                }
                _ => "Usage: discord liverole <add|remove> [args...]".to_string(),
            }
        }
        
        "event" => {
            if args.len() < 2 {
                return "Usage: discord event (list|addrole|delrole) [args...]".to_string();
            }
            
            match args[1].to_lowercase().as_str() {
                "list" => {
                    let guild_id = if args.len() > 2 { Some(args[2]) } else { None };
                    
                    match DiscordCommands::list_event_configs(client, guild_id).await {
                        Ok(result) => {
                            if result.data.configs.is_empty() {
                                "No Discord event configs found.".to_string()
                            } else {
                                let mut out = String::from("Discord event configs:\n");
                                for config in result.data.configs {
                                    out.push_str(&format!(
                                        " - Event: '{}', Guild: '{}', Roles: {:?}, Enabled: {}\n",
                                        config.event_name,
                                        config.guild_id,
                                        config.role_ids,
                                        config.is_enabled
                                    ));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing event configs: {}", e),
                    }
                }
                "addrole" => {
                    if args.len() < 5 {
                        return "Usage: discord event addrole <eventName> <roleId> <guildId>".to_string();
                    }
                    let event_name = args[2];
                    let role_id = args[3];
                    let guild_id = args[4];
                    
                    match DiscordCommands::add_event_role(client, event_name, role_id, guild_id).await {
                        Ok(_) => format!("Added role {} to event '{}' in guild {}.", role_id, event_name, guild_id),
                        Err(e) => format!("Error adding role: {}", e),
                    }
                }
                "delrole" => {
                    if args.len() < 4 {
                        return "Usage: discord event delrole <eventName> <roleId>".to_string();
                    }
                    let event_name = args[2];
                    let role_id = args[3];
                    
                    match DiscordCommands::remove_event_role(client, event_name, role_id).await {
                        Ok(_) => format!("Removed role {} from event '{}'.", role_id, event_name),
                        Err(e) => format!("Error removing role: {}", e),
                    }
                }
                _ => "Usage: discord event (list|addrole|delrole) [args...]".to_string(),
            }
        }
        
        "send" => {
            if args.len() < 3 {
                return "Usage: discord send <channelId> <message>".to_string();
            }
            let channel_id = args[1];
            let message = args[2..].join(" ");
            
            let account_name = match get_connected_discord_account(client).await {
                Ok(name) => name,
                Err(e) => return e,
            };
            
            match DiscordCommands::send_message(client, &account_name, channel_id, &message).await {
                Ok(result) => format!("Message sent (ID: {})", result.data.message_id),
                Err(e) => format!("Error sending message: {}", e),
            }
        }
        
        "member" => {
            if args.len() < 3 {
                return "Usage: discord member <guildId> <userId>".to_string();
            }
            let guild_id = args[1];
            let user_id = args[2];
            
            let account_name = match get_connected_discord_account(client).await {
                Ok(name) => name,
                Err(e) => return e,
            };
            
            match DiscordCommands::get_member(client, &account_name, guild_id, user_id).await {
                Ok(result) => {
                    let member = &result.data.member;
                    let mut out = format!("Discord Member Info:\n");
                    out.push_str(&format!(" - User: {} (ID: {})\n", member.username, member.user_id));
                    out.push_str(&format!(" - Display Name: {}\n", member.display_name));
                    if let Some(joined_at) = &member.joined_at {
                        out.push_str(&format!(" - Joined: {}\n", joined_at.seconds));
                    }
                    if !member.role_ids.is_empty() {
                        out.push_str(&format!(" - Roles: {}\n", member.role_ids.join(", ")));
                    }
                    out
                }
                Err(e) => format!("Error getting member: {}", e),
            }
        }
        
        // Legacy command aliases for backward compatibility
        "guilds" => {
            Box::pin(handle_discord_command(&["list", "guilds"], client)).await
        }
        "channels" => {
            let mut new_args = vec!["list", "channels"];
            new_args.extend_from_slice(&args[1..]);
            Box::pin(handle_discord_command(&new_args, client)).await
        }
        "roles" => {
            let mut new_args = vec!["list", "roles"];
            new_args.extend_from_slice(&args[1..]);
            Box::pin(handle_discord_command(&new_args, client)).await
        }
        "members" => {
            let mut new_args = vec!["list", "members"];
            new_args.extend_from_slice(&args[1..]);
            Box::pin(handle_discord_command(&new_args, client)).await
        }
        "msg" => {
            let mut new_args = vec!["send"];
            new_args.extend_from_slice(&args[1..]);
            Box::pin(handle_discord_command(&new_args, client)).await
        }
        
        _ => show_usage(),
    }
}

fn show_usage() -> String {
    "Discord Commands:
  discord list guilds - List all Discord guilds the bot is in
  discord list channels [guildId] - List channels (auto-detects guild if only one)
  discord list roles [guildId] - List roles (auto-detects guild if only one)
  discord list members [guildId] - List members (auto-detects guild if only one)
  discord list liveroles - List all live role configurations
  
  discord event list [guildId] - List Discord event configurations
  discord event addrole <eventName> <roleId> <guildId> - Add role to event
  discord event delrole <eventName> <roleId> - Remove role from event
  
  discord liverole add <guildId> <roleId> - Set role to assign when streaming
  discord liverole remove <guildId> - Remove live role configuration
  
  discord send <channelId> <message> - Send a message to a channel
  discord member <guildId> <userId> - Get info about a member
  
Legacy aliases (still work):
  discord guilds = discord list guilds
  discord channels = discord list channels
  discord roles = discord list roles
  discord members = discord list members
  discord msg = discord send".to_string()
}