// Simplified TTV (Twitch) command adapter for TUI without TuiModule dependency
use maowbot_common_ui::{GrpcClient, commands::twitch::TwitchCommands};

// TODO: This should be configurable or obtained from a service
const DEFAULT_TWITCH_ACCOUNT: &str = "default";

pub async fn handle_ttv_command(
    args: &[&str],
    client: &GrpcClient,
) -> String {
    if args.is_empty() {
        return ttv_usage();
    }

    match args[0].to_lowercase().as_str() {
        "msg" => {
            if args.len() < 3 {
                return "Usage: ttv msg <channel> <text>".to_string();
            }
            let channel = args[1];
            let text = args[2..].join(" ");
            
            match TwitchCommands::send_message(client, DEFAULT_TWITCH_ACCOUNT, channel, &text).await {
                Ok(result) => {
                    if let Some(warning) = result.warnings.first() {
                        format!("Warning: {}", warning)
                    } else {
                        "Message sent.".to_string()
                    }
                }
                Err(e) => format!("Error sending message: {}", e),
            }
        }
        
        "info" => {
            if args.len() < 2 {
                return "Usage: ttv info <channel>".to_string();
            }
            let channel = args[1];
            
            match TwitchCommands::get_channel_info(client, channel).await {
                Ok(result) => {
                    let info = &result.data.channel;
                    let mut out = format!("Channel: {}\n", info.display_name);
                    out.push_str(&format!("Title: {}\n", info.title));
                    out.push_str(&format!("Game: {}\n", info.game_name));
                    out.push_str(&format!("Language: {}\n", info.language));
                    out
                }
                Err(e) => format!("Error getting channel info: {}", e),
            }
        }
        
        "follow" => {
            if args.len() < 2 {
                return "Usage: ttv follow <channel>".to_string();
            }
            let channel = args[1];
            
            // Follow/unfollow not available in current API
            "Follow command not implemented".to_string()
            /*match TwitchCommands::follow_channel(client, DEFAULT_TWITCH_ACCOUNT, channel).await {
                Ok(_) => format!("Followed channel '{}'.", channel),
                Err(e) => format!("Error following channel: {}", e),
            }*/
        }
        
        "unfollow" => {
            if args.len() < 2 {
                return "Usage: ttv unfollow <channel>".to_string();
            }
            let channel = args[1];
            
            // Follow/unfollow not available in current API
            "Unfollow command not implemented".to_string()
            /*match TwitchCommands::unfollow_channel(client, DEFAULT_TWITCH_ACCOUNT, channel).await {
                Ok(_) => format!("Unfollowed channel '{}'.", channel),
                Err(e) => format!("Error unfollowing channel: {}", e),
            }*/
        }
        
        "stream" => {
            if args.len() < 2 {
                return "Usage: ttv stream <channel>".to_string();
            }
            let channel = args[1];
            
            match TwitchCommands::get_stream_info(client, channel).await {
                Ok(result) => {
                    if let Some(stream) = result.data.stream {
                        let mut out = format!("Stream is LIVE!\n");
                        out.push_str(&format!("Title: {}\n", stream.title));
                        out.push_str(&format!("Game: {}\n", stream.game_name));
                        out.push_str(&format!("Viewers: {}\n", stream.viewer_count));
                        if let Some(started) = &stream.started_at {
                            out.push_str(&format!("Started: {} seconds\n", started.seconds));
                        }
                        out
                    } else {
                        format!("Channel '{}' is offline.", channel)
                    }
                }
                Err(e) => format!("Error getting stream info: {}", e),
            }
        }
        
        "ban" => {
            if args.len() < 3 {
                return "Usage: ttv ban <channel> <username> [reason]".to_string();
            }
            let channel = args[1];
            let username = args[2];
            let reason = if args.len() > 3 { args[3..].join(" ") } else { String::new() };
            
            // Ban/unban not available in current API
            "Ban command not implemented".to_string()
            /*match TwitchCommands::ban_user(client, DEFAULT_TWITCH_ACCOUNT, channel, username, &reason).await {
                Ok(_) => format!("Banned user '{}' from channel '{}'.", username, channel),
                Err(e) => format!("Error banning user: {}", e),
            }*/
        }
        
        "unban" => {
            if args.len() < 3 {
                return "Usage: ttv unban <channel> <username>".to_string();
            }
            let channel = args[1];
            let username = args[2];
            
            // Ban/unban not available in current API
            "Unban command not implemented".to_string()
            /*match TwitchCommands::unban_user(client, DEFAULT_TWITCH_ACCOUNT, channel, username).await {
                Ok(_) => format!("Unbanned user '{}' from channel '{}'.", username, channel),
                Err(e) => format!("Error unbanning user: {}", e),
            }*/
        }
        
        _ => ttv_usage(),
    }
}

fn ttv_usage() -> String {
    "Twitch Commands:
  ttv msg <channel> <text> - Send a message to a channel
  ttv info <channel> - Get channel information
  ttv follow <channel> - Follow a channel
  ttv unfollow <channel> - Unfollow a channel
  ttv stream <channel> - Get stream information
  ttv ban <channel> <username> [reason] - Ban a user
  ttv unban <channel> <username> - Unban a user".to_string()
}