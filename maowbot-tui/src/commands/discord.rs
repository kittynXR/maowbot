use std::sync::Arc;
use maowbot_common::traits::api::BotApi;

/// Handle "discord" TUI commands.
/// We focus on just:
///   - `discord list guilds`
///   - `discord list channels <guildId>`
pub async fn handle_discord_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return show_usage();
    }

    match args[0].to_lowercase().as_str() {
        "list" => {
            // usage: discord list (guilds|channels)
            if args.len() < 2 {
                return "Usage: discord list (guilds|channels)".to_string();
            }
            match args[1].to_lowercase().as_str() {
                "guilds" => {
                    match bot_api.list_discord_guilds("cutecat_chat").await {
                        Ok(guilds) => {
                            if guilds.is_empty() {
                                "No guilds found from the Twilight cache.".to_string()
                            } else {
                                let mut out = String::new();
                                out.push_str("Discord Guilds (from in-memory cache):\n");
                                for g in guilds {
                                    out.push_str(&format!("  - {} (ID={})\n", g.guild_name, g.guild_id));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing guilds: {e}"),
                    }
                }
                "channels" => {
                    // usage: discord list channels <guildId>
                    if args.len() < 3 {
                        return "Usage: discord list channels <guildId>".to_string();
                    }
                    let guild_id = args[2];
                    match bot_api.list_discord_channels("cutecat_chat", guild_id).await {
                        Ok(channels) => {
                            if channels.is_empty() {
                                format!("No channels found for guild '{guild_id}'.")
                            } else {
                                let mut out = format!("Channels in guild '{guild_id}':\n");
                                for ch in channels {
                                    out.push_str(&format!("  - {} (ID={})\n", ch.channel_name, ch.channel_id));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing channels: {e}"),
                    }
                }
                _ => "Usage: discord list (guilds|channels)".to_string(),
            }
        }

        _ => show_usage(),
    }
}

fn show_usage() -> String {
    r#"Discord Commands:
  discord list guilds
  discord list channels <guildId>
"#
        .to_string()
}
