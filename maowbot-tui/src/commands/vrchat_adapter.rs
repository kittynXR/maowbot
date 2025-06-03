// VRChat command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::vrchat::VRChatCommands};

pub async fn handle_vrchat_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return show_vrchat_usage();
    }

    match args[0] {
        "world" => {
            let account_name = if args.len() >= 2 { args[1] } else { "" };
            match VRChatCommands::get_current_world(client, account_name).await {
                Ok(world) => format_world_info(&world),
                Err(e) => format!("Error => {}", e),
            }
        }
        "avatar" => {
            if args.len() == 1 {
                let account_name = "";
                return match VRChatCommands::get_current_avatar(client, account_name).await {
                    Ok(av) => format_avatar_info(&av),
                    Err(e) => format!("Error => {}", e),
                };
            } else if args.len() == 2 && !args[1].eq_ignore_ascii_case("change") {
                let account_name = args[1];
                return match VRChatCommands::get_current_avatar(client, account_name).await {
                    Ok(av) => format_avatar_info(&av),
                    Err(e) => format!("Error => {}", e),
                };
            } else if args.len() == 3 && args[1].eq_ignore_ascii_case("change") {
                // e.g. "vrchat avatar change <id>"
                let new_id = args[2];
                let account_name = "";
                return match VRChatCommands::change_avatar(client, account_name, new_id).await {
                    Ok(_) => format!("Avatar changed to {}", new_id),
                    Err(e) => format!("Error => {}", e),
                };
            } else if args.len() == 4 && args[2].eq_ignore_ascii_case("change") {
                // e.g. "vrchat avatar kittyn change <id>"
                let account_name = args[1];
                let new_id = args[3];
                return match VRChatCommands::change_avatar(client, account_name, new_id).await {
                    Ok(_) => format!("Avatar changed to {}", new_id),
                    Err(e) => format!("Error => {}", e),
                };
            } else {
                return show_vrchat_usage();
            }
        }
        "instance" => {
            // "vrchat instance [accountName]"
            let account_name = if args.len() >= 2 { args[1] } else { "" };
            match VRChatCommands::get_current_instance(client, account_name).await {
                Ok(instance_data) => format_instance_info(&instance_data),
                Err(e) => format!("Error => {}", e),
            }
        }
        "account" => {
            if args.len() < 2 {
                return "Usage: vrchat account <accountName>".to_string();
            }
            let acct_name = args[1];
            match VRChatCommands::set_vrchat_account(client, acct_name).await {
                Ok(_) => format!("VRChat active account set to '{}'.", acct_name),
                Err(e) => format!("Error setting vrchat account => {}", e),
            }
        }
        _ => show_vrchat_usage(),
    }
}

fn format_world_info(world: &maowbot_common_ui::commands::vrchat::VRChatWorldInfo) -> String {
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

fn format_avatar_info(avatar: &maowbot_common_ui::commands::vrchat::VRChatAvatarInfo) -> String {
    let mut out = String::new();
    out.push_str("Avatar Name: ");
    out.push_str(&avatar.avatar_name);
    out.push_str("\nAvatar ID: ");
    out.push_str(&avatar.avatar_id);
    out
}

fn format_instance_info(i: &maowbot_common_ui::commands::vrchat::VRChatInstanceInfo) -> String {
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