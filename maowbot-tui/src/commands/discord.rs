use std::sync::Arc;
use maowbot_common::error::Error;
use maowbot_common::traits::api::BotApi;

/// Handle "discord" commands in the TUI.
/// Supported subcommands:
///   1) discord list guilds
///   2) discord list channels <guildId>
///   3) discord list commands
///   4) discord send message <guildId> <channelId> [message text...]
pub async fn handle_discord_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return show_usage();
    }

    match args[0].to_lowercase().as_str() {
        "list" => {
            // Usage: discord list (guilds|channels|commands)
            if args.len() < 2 {
                return "Usage: discord list (guilds|channels|commands)".to_string();
            }
            match args[1].to_lowercase().as_str() {
                "guilds" => {
                    // We'll assume a single ephemeral account, e.g. "ephemeral-bot"
                    match bot_api.list_discord_guilds("ephemeral-bot").await {
                        Ok(guilds) => {
                            if guilds.is_empty() {
                                "No guilds found.".to_string()
                            } else {
                                let mut out = String::new();
                                out.push_str("Discord Guilds:\n");
                                for g in guilds {
                                    out.push_str(&format!(" - {} (ID={})\n", g.guild_name, g.guild_id));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing guilds: {e}"),
                    }
                }
                "channels" => {
                    // Usage: discord list channels <guildId>
                    if args.len() < 3 {
                        return "Usage: discord list channels <guildId>".to_string();
                    }
                    let guild_id = args[2];
                    match bot_api.list_discord_channels("ephemeral-bot", guild_id).await {
                        Ok(channels) => {
                            if channels.is_empty() {
                                format!("No channels found for guild '{guild_id}'.")
                            } else {
                                let mut out = format!("Channels in guild '{guild_id}':\n");
                                for ch in channels {
                                    out.push_str(&format!(" - {} (ID={})\n", ch.channel_name, ch.channel_id));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing channels: {e}"),
                    }
                }
                "commands" => {
                    match bot_api.list_discord_commands("ephemeral-bot").await {
                        Ok(cmds) => {
                            if cmds.is_empty() {
                                "No Discord commands found.".to_string()
                            } else {
                                let mut out = String::from("Discord Commands:\n");
                                for (cmd_id, cmd_name) in cmds {
                                    out.push_str(&format!(" - {} (ID={})\n", cmd_name, cmd_id));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing commands: {e}"),
                    }
                }
                _ => "Usage: discord list (guilds|channels|commands)".to_string(),
            }
        }

        "send" => {
            // Usage: discord send message <guildId> <channelId> [text...]
            if args.len() < 4 {
                return "Usage: discord send message <guildId> <channelId> [text...]".to_string();
            }
            if args[1].to_lowercase() != "message" {
                return "Usage: discord send message <guildId> <channelId> [text...]".to_string();
            }
            let guild_id = args[2];
            let channel_id = args[3];
            let message_text = if args.len() > 4 {
                args[4..].join(" ")
            } else {
                "[Empty message]".to_string()
            };

            // We call our newly added method:
            match bot_api.send_discord_message("ephemeral-bot", guild_id, channel_id, &message_text).await {
                Ok(_) => format!("Sent message to channel '{channel_id}' in guild '{guild_id}'."),
                Err(e) => format!("Error sending message: {e}"),
            }
        }

        _ => show_usage(),
    }
}

fn show_usage() -> String {
    r#"Discord Commands:
  discord list guilds
  discord list channels <guildId>
  discord list commands
  discord send message <guildId> <channelId> [messagetext]
"#
        .to_string()
}
