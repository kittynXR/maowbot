// File: maowbot-tui/src/commands/vrchat.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::{BotApi, VRChatWorldBasic, VRChatAvatarBasic};

/// Handle "vrchat" subcommands from TUI.
/// If the user typed e.g. "vrchat world" with no name, we pass "" to the BotApi
/// so it knows to do the single-account logic.
pub async fn handle_vrchat_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return show_vrchat_usage();
    }

    match args[0] {
        "world" => {
            // Possible usage:
            //   vrchat world              -> if exactly 1 VRChat account, use it
            //   vrchat world kittyn       -> use that account
            let account_name = if args.len() >= 2 { args[1] } else { "" };
            match bot_api.vrchat_get_current_world(account_name).await {
                Ok(world) => format_world_info(&world),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "avatar" => {
            // usage can be:
            //   vrchat avatar
            //   vrchat avatar kittyn
            //   vrchat avatar kittyn change <id>
            //   vrchat avatar change <id>  (if exactly 1 account)
            //
            // We will see if "change" is in there. The easiest is:
            if args.len() == 1 {
                // "vrchat avatar"
                let account_name = "";
                return match bot_api.vrchat_get_current_avatar(account_name).await {
                    Ok(av) => format_avatar_info(&av),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else if args.len() == 2 && !args[1].eq_ignore_ascii_case("change") {
                // "vrchat avatar kittyn"
                let account_name = args[1];
                return match bot_api.vrchat_get_current_avatar(account_name).await {
                    Ok(av) => format_avatar_info(&av),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else if args.len() == 3 && args[1].eq_ignore_ascii_case("change") {
                // "vrchat avatar change <id>" (no account name, single acct?)
                let new_avatar_id = args[2];
                let account_name = "";
                return match bot_api.vrchat_change_avatar(account_name, new_avatar_id).await {
                    Ok(_) => format!("Avatar changed to {}", new_avatar_id),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else if args.len() == 4 && args[2].eq_ignore_ascii_case("change") {
                // "vrchat avatar kittyn change <id>"
                let account_name = args[1];
                let new_avatar_id = args[3];
                return match bot_api.vrchat_change_avatar(account_name, new_avatar_id).await {
                    Ok(_) => format!("Avatar changed to {}", new_avatar_id),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else {
                return show_vrchat_usage();
            }
        }
        _ => show_vrchat_usage(),
    }
}

fn show_vrchat_usage() -> String {
    r#"Usage:
  vrchat world [accountName]
    - fetches the current world from VRChat, ignoring the runtime

  vrchat avatar [accountName]
    - fetches the current avatar

  vrchat avatar [accountName] change <avatarId>
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