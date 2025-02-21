// File: maowbot-tui/src/commands/vrchat.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::{BotApi, VRChatWorldBasic, VRChatAvatarBasic};

/// Handle "vrchat" subcommands from TUI.
pub async fn handle_vrchat_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return show_vrchat_usage();
    }

    match args[0] {
        "world" => {
            if args.len() != 2 {
                // usage: vrchat world <accountName>
                return "Usage: vrchat world <accountName>".to_string();
            }
            let account_name = args[1];
            match bot_api.vrchat_get_current_world(account_name).await {
                Ok(world) => format_world_info(&world),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "avatar" => {
            // usage:
            //   vrchat avatar <accountName>
            //   vrchat avatar <accountName> change <avatarId>
            if args.len() < 2 {
                return "Usage: vrchat avatar <accountName> [change <avatarId>]".to_string();
            }
            let account_name = args[1];
            if args.len() == 2 {
                // just show current avatar
                match bot_api.vrchat_get_current_avatar(account_name).await {
                    Ok(av) => format_avatar_info(&av),
                    Err(e) => format!("Error => {:?}", e),
                }
            } else {
                // maybe "change <avatarId>"
                if args.len() == 4 && args[2].eq_ignore_ascii_case("change") {
                    let avatar_id = args[3];
                    match bot_api.vrchat_change_avatar(account_name, avatar_id).await {
                        Ok(_) => format!("Avatar changed to {}", avatar_id),
                        Err(e) => format!("Error => {:?}", e),
                    }
                } else {
                    "Usage: vrchat avatar <accountName> [change <avatarId>]".to_string()
                }
            }
        }
        _ => show_vrchat_usage(),
    }
}

fn show_vrchat_usage() -> String {
    r#"Usage:
  vrchat world <accountName>
    - fetches the current world from VRChat API and prints details

  vrchat avatar <accountName>
    - fetches the current avatar and prints info

  vrchat avatar <accountName> change <avatarId>
    - changes your avatar to the specified avatarId
"#
        .to_string()
}

fn format_world_info(world: &VRChatWorldBasic) -> String {
    let mut out = String::new();
    out.push_str(&format!("World Name: {}\n", world.name));
    out.push_str(&format!("Author Name: {}\n", world.author_name));
    out.push_str(&format!("Last Updated: {}\n", world.updated_at));
    out.push_str(&format!("Date Published: {}\n", world.created_at));
    out.push_str(&format!("Max Capacity: {}\n", world.capacity));
    out
}

fn format_avatar_info(avatar: &VRChatAvatarBasic) -> String {
    let mut out = String::new();
    out.push_str(&format!("Avatar Name: {}\n", avatar.avatar_name));
    out.push_str(&format!("Avatar ID: {}\n", avatar.avatar_id));
    out
}