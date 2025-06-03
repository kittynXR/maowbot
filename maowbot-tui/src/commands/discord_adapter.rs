// Discord command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::discord::DiscordCommands};

// TODO: This should be configurable or obtained from a service
const DEFAULT_DISCORD_ACCOUNT: &str = "default";

pub async fn handle_discord_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return show_usage();
    }

    match args[0].to_lowercase().as_str() {
        "liverole" => {
            if args.len() < 2 {
                return "Usage: discord liverole <guildId> <roleId> OR discord liverole list OR discord liverole remove <guildId>".to_string();
            }
            
            match args[1].to_lowercase().as_str() {
                "list" => {
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
                _ => {
                    // Assume it's "discord liverole <guildId> <roleId>"
                    if args.len() < 3 {
                        return "Usage: discord liverole <guildId> <roleId>".to_string();
                    }
                    let guild_id = args[1];
                    let role_id = args[2];
                    
                    match DiscordCommands::set_live_role(client, guild_id, role_id).await {
                        Ok(_) => format!(
                            "Set live role: Guild {} will assign role {} to users who are streaming on Twitch.",
                            guild_id,
                            role_id
                        ),
                        Err(e) => format!("Error setting live role: {}", e),
                    }
                }
            }
        }
        
        "guilds" => {
            match DiscordCommands::list_guilds(client, DEFAULT_DISCORD_ACCOUNT).await {
                Ok(result) => {
                    if result.data.guilds.is_empty() {
                        "Bot is not in any Discord guilds.".to_string()
                    } else {
                        let mut out = String::from("Discord guilds:\n");
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
            if args.len() < 2 {
                return "Usage: discord channels <guildId>".to_string();
            }
            let guild_id = args[1];
            
            match DiscordCommands::list_channels(client, DEFAULT_DISCORD_ACCOUNT, guild_id).await {
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
        
        "send" => {
            if args.len() < 3 {
                return "Usage: discord send <channelId> <message>".to_string();
            }
            let channel_id = args[1];
            let message = args[2..].join(" ");
            
            match DiscordCommands::send_message(client, DEFAULT_DISCORD_ACCOUNT, channel_id, &message).await {
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
            
            match DiscordCommands::get_member(client, DEFAULT_DISCORD_ACCOUNT, guild_id, user_id).await {
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
        
        "members" => {
            if args.len() < 2 {
                return "Usage: discord members <guildId>".to_string();
            }
            let guild_id = args[1];
            
            match DiscordCommands::list_members(client, DEFAULT_DISCORD_ACCOUNT, guild_id, 50).await {
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
        
        _ => show_usage(),
    }
}

fn show_usage() -> String {
    "Discord Commands:
  discord liverole <guildId> <roleId> - Add live role (assigned when streaming)
  discord liverole list - List all live role configurations
  discord liverole remove <guildId> - Remove live role configuration
  discord guilds - List all Discord guilds the bot is in
  discord channels <guildId> - List channels in a guild
  discord send <channelId> <message> - Send a message to a channel
  discord member <guildId> <userId> - Get info about a member
  discord members <guildId> - List members in a guild".to_string()
}