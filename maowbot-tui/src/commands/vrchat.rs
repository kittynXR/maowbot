use std::sync::Arc;
use maowbot_core::Error;
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::{BotApi};
use maowbot_core::plugins::bot_api::vrchat_api::{
    VRChatAvatarBasic, VRChatWorldBasic, VRChatInstanceBasic,
};

pub async fn handle_vrchat_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return show_vrchat_usage();
    }

    match args[0] {
        "world" => {
            let account_name = if args.len() >= 2 { args[1] } else { "" };
            match bot_api.vrchat_get_current_world(account_name).await {
                Ok(world) => format_world_info(&world),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "avatar" => {
            if args.len() == 1 {
                let account_name = "";
                return match bot_api.vrchat_get_current_avatar(account_name).await {
                    Ok(av) => format_avatar_info(&av),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else if args.len() == 2 && !args[1].eq_ignore_ascii_case("change") {
                let account_name = args[1];
                return match bot_api.vrchat_get_current_avatar(account_name).await {
                    Ok(av) => format_avatar_info(&av),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else if args.len() == 3 && args[1].eq_ignore_ascii_case("change") {
                // e.g. "vrchat avatar change <id>"
                let new_id = args[2];
                let account_name = "";
                return match bot_api.vrchat_change_avatar(account_name, new_id).await {
                    Ok(_) => format!("Avatar changed to {}", new_id),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else if args.len() == 4 && args[2].eq_ignore_ascii_case("change") {
                // e.g. "vrchat avatar kittyn change <id>"
                let account_name = args[1];
                let new_id = args[3];
                return match bot_api.vrchat_change_avatar(account_name, new_id).await {
                    Ok(_) => format!("Avatar changed to {}", new_id),
                    Err(e) => format!("Error => {:?}", e),
                };
            } else {
                return show_vrchat_usage();
            }
        }
        "instance" => {
            // "vrchat instance [accountName]"
            let account_name = if args.len() >= 2 { args[1] } else { "" };
            match bot_api.vrchat_get_current_instance(account_name).await {
                Ok(instance_data) => format_instance_info(&instance_data),
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "account" => {
            if args.len() < 2 {
                return "Usage: vrchat account <accountName>".to_string();
            }
            let acct_name = args[1];
            match set_vrchat_account(bot_api, acct_name).await {
                Ok(_) => format!("VRChat active account set to '{}'.", acct_name),
                Err(e) => format!("Error setting vrchat account => {e}"),
            }
        }
        _ => show_vrchat_usage(),
    }
}

fn format_world_info(world: &VRChatWorldBasic) -> String {
    let mut out = String::new();
    out.push_str("World Name: ");
    out.push_str(&world.name);
    out.push_str("\nAuthor Name: ");
    out.push_str(&world.author_name);
    out.push_str("\nRelease Status: ");
    out.push_str(&world.release_status);
    out.push_str("\nMax Capacity: ");
    out.push_str(&world.capacity.to_string());
    out.push_str("\nDate Published: ");
    out.push_str(&world.created_at);
    out.push_str("\nLast Updated: ");
    out.push_str(&world.updated_at);
    if !world.description.is_empty() {
        out.push_str("\n\nDescription:\n");
        out.push_str(&world.description);
    }
    out
}

fn format_avatar_info(avatar: &VRChatAvatarBasic) -> String {
    let mut out = String::new();
    out.push_str("Avatar Name: ");
    out.push_str(&avatar.avatar_name);
    out.push_str("\nAvatar ID: ");
    out.push_str(&avatar.avatar_id);
    out
}

fn format_instance_info(i: &VRChatInstanceBasic) -> String {
    let mut out = String::new();
    out.push_str("Instance Data:\n");
    out.push_str("  world_id:    ");
    out.push_str(&i.world_id.clone().unwrap_or_default());
    out.push_str("\n  instance_id: ");
    out.push_str(&i.instance_id.clone().unwrap_or_default());
    out.push_str("\n  location:    ");
    out.push_str(&i.location.clone().unwrap_or_default());
    out
}

/// Sets the `vrchat_active_account` in bot_config if the given account name
/// is valid (i.e. we have a VRChat credential for it).
///
/// - The `<accountName>` must be a user name for which we have a VRChat credential (matching user_name).
async fn set_vrchat_account(bot_api: &Arc<dyn BotApi>, account_name: &str) -> Result<(), Error> {
    // 1) Ensure we have at least one VRChat credential with user_name == account_name (case-insensitive).
    let all_vrc = bot_api.list_credentials(Some(Platform::VRChat)).await?;
    let found = all_vrc.iter().any(|c| c.user_name.eq_ignore_ascii_case(account_name));
    if !found {
        return Err(Error::Platform(format!(
            "No VRChat credential found with user_name='{}'. Try 'account add vrchat' first.",
            account_name
        )));
    }

    // 2) If that’s valid, store in bot_config so the built-in commands can read it.
    bot_api.set_bot_config_value("vrchat_active_account", account_name).await?;
    Ok(())
}

fn show_vrchat_usage() -> String {
    r#"Usage:
  vrchat world [accountName]
    - fetches current world from VRChat

  vrchat avatar [accountName]
    - fetches current avatar
  vrchat avatar [accountName] change <avatarId>
    - changes your avatar to the specified avatarId

  vrchat instance [accountName]
    - fetches the user's current (world + instance)

  vrchat account <accountName>
    - sets the default VRChat account for built-in commands
"#
        .to_string()
}