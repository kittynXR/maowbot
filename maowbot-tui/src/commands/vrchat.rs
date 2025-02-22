use std::sync::Arc;
use maowbot_core::Error;
use maowbot_core::plugins::bot_api::{
    BotApi,
    VRChatWorldBasic,
    VRChatAvatarBasic
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
                // e.g. "vrchat avatar kittyn"
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
                Ok(instance_data) => {
                    let mut out = String::new();
                    out.push_str("Instance Data:\n");
                    out.push_str(&format!("  world_id:    {:?}\n", instance_data.world_id));
                    out.push_str(&format!("  instance_id: {:?}\n", instance_data.instance_id));
                    out.push_str(&format!("  location:    {:?}\n", instance_data.location));
                    out
                }
                Err(e) => format!("Error => {:?}", e),
            }
        }
        _ => show_vrchat_usage(),
    }
}

// You might prefer to add this logic to BotApi or a separate function:
async fn fetch_current_instance_direct(
    bot_api: &Arc<dyn BotApi>,
    account_name: &str
) -> Result<Option<(String, String)>, Box<dyn std::error::Error>>
{
    // 1) Find the user in the DB / get the VRChat credential
    // 2) Use a function that calls your newly added VRChatClient::fetch_current_instance_api(...)

    // This is a pseudo‐code approach. If you have a direct bot_api method for instance, call that.
    // For brevity, let's do a direct call:

    use maowbot_core::platforms::vrchat::client::VRChatClient;
    // We do nearly the same steps as "vrchat_get_current_world" but for instance

    // 1) If account_name is blank, do single‐credential logic:
    let name_to_use = if account_name.is_empty() {
        // re‐use the logic from your existing code...
        let all_creds = bot_api.list_credentials(Some(maowbot_core::models::Platform::VRChat)).await?;
        if all_creds.is_empty() {
            return Err(Box::new(Error::Platform("No VRChat credentials.".into())));
        }
        if all_creds.len() > 1 {
            return Err(Box::new(Error::Platform(
                "Multiple VRChat accounts found. Provide an accountName.".into()
            )));
        }
        let c = &all_creds[0];
        // fetch user from DB
        let user_opt = bot_api.get_user(c.user_id).await?;
        match user_opt {
            Some(u) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
            None => c.user_id.to_string(),
        }
    } else {
        account_name.to_string()
    };

    // 2) Look up the user record
    let user = bot_api.find_user_by_name(&name_to_use).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    // 3) find VRChat creds
    let cred_opt = bot_api.list_credentials(Some(maowbot_core::models::Platform::VRChat))
        .await?
        .into_iter()
        .find(|c| c.user_id == user.user_id);

    let cred = match cred_opt {
        Some(c) => c,
        None => {
            return Err(Box::new(Error::Platform(
                format!("No VRChat credential for '{name_to_use}'")
            )));
        }
    };
    let session_cookie = cred.primary_token.clone();

    // 4) Also you must know the user’s VRChat ID. 
    //    If you store it anywhere, pull it from additional_data or from a separate table.
    //    For brevity here, let's guess you store it in cred.platform_id:
    let user_vrc_id = match cred.platform_id {
        Some(ref v) if !v.is_empty() => v.clone(),
        _ => {
            return Err(Box::new(Error::Platform(
                "Missing VRChat userId in credential.platform_id".into()
            )));
        }
    };

    // 5) Make the client & call fetch_current_instance_api
    let client = VRChatClient::new(&session_cookie)?;
    match client.fetch_current_instance_api().await {
        Ok(Some(inst)) => {
            Ok(Some((inst.world_id.unwrap(), inst.instance_id.unwrap())))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(Box::new(e)),
    }
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
"#
        .to_string()
}