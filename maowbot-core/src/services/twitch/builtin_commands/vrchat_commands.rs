use crate::Error;
use crate::platforms::vrchat::client::VRChatClient;
use crate::services::twitch::command_service::CommandContext;
use tracing::{info, warn};
use maowbot_common::models::Command;
use maowbot_common::models::platform::Platform;
use maowbot_common::models::user::User;

/// Helper function: if we can parse an ISO8601 string (like `2024-05-13T04:04:20.108Z`),
/// convert it to `YYYY-MM-DD`. Otherwise return the original string.
fn short_ymd(date_str: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(date_str) {
        Ok(dt) => dt.format("%Y-%m-%d").to_string(),
        Err(_) => date_str.to_string(),
    }
}

/// handle_world is invoked for the `!world` command.
///
/// It retrieves VRChat world info, then outputs:
/// - one message with name, author, capacity, release status, published date, and last updated (YYYY-MM-DD).
/// - one or more messages for the description if present, chunking if necessary.
pub async fn handle_world(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    _user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    // 1) Determine which VRChat account to use from bot_config
    let configured_account = match ctx.bot_config_repo.get_value("vrchat_active_account").await? {
        Some(val) if !val.trim().is_empty() => val,
        _ => "broadcaster".to_string(),
    };
    info!("handle_world => VRChat account from config: '{}'", configured_account);

    // 2) Find that VRChat credential
    let all_vrc_creds = ctx.credentials_repo.list_credentials_for_platform(&Platform::VRChat).await?;
    let vrc_cred_opt = all_vrc_creds
        .into_iter()
        .find(|c| c.user_name.eq_ignore_ascii_case(&configured_account));

    let cred = match vrc_cred_opt {
        Some(c) => c,
        None => {
            return Ok(format!(
                "No VRChat credentials found for account '{}'. \
Please set 'vrchat_active_account' or run 'account add vrchat'.",
                configured_account
            ));
        }
    };

    // 3) Fetch the current world info
    let client = VRChatClient::new(&cred.primary_token)?;
    let winfo_opt = client.fetch_current_world_api().await?;
    if winfo_opt.is_none() {
        return Ok("User is offline or not in any world.".to_string());
    }
    let w = winfo_opt.unwrap();

    // 4) Convert published/updated fields to short YYYY-MM-DD if possible
    let published_str = w
        .published_at
        .as_deref()
        .map(short_ymd)
        .unwrap_or_else(|| "(unknown)".to_string());
    let updated_str = w
        .updated_at
        .as_deref()
        .map(short_ymd)
        .unwrap_or_else(|| "(unknown)".to_string());

    // 5) Prepare the first message
    let release_str = w.release_status.clone().unwrap_or_default();
    let first_message = format!(
        "[World Info] Name: {} | Author: {} | Capacity: {} | Status: {} | \
Published: {} | Last Updated: {}",
        w.name.trim(),
        w.author_name.trim(),
        w.capacity,
        release_str.trim(),
        published_str,   // already short-ymd
        updated_str      // already short-ymd
    );

    // 6) Next, handle the description (in separate messages, chunked if too long)
    let mut results = vec![first_message];
    if let Some(desc) = w.description {
        let desc_clean = desc.trim();
        if !desc_clean.is_empty() {
            let max_len = 400; // Reduced from 450 to be safer with multi-byte characters
            let mut remaining = desc_clean;
            let mut first_chunk = true;
            while !remaining.is_empty() {
                // Take as many bytes as possible without exceeding max_len
                let chunk_size = if remaining.len() <= max_len {
                    remaining.len() 
                } else {
                    // Find the last valid char boundary before max_len
                    let mut size = max_len;
                    while !remaining.is_char_boundary(size) && size > 0 {
                        size -= 1;
                    }
                    size
                };
                
                let chunk_text = &remaining[..chunk_size];
                let prefix = if first_chunk {
                    first_chunk = false;
                    "Description: "
                } else {
                    ""
                };
                results.push(format!("{}{}", prefix, chunk_text));
                remaining = &remaining[chunk_size..];
            }
        }
    }

    Ok(results.join("<SPLIT>"))
}

/// handle_instance is invoked for the `!instance` command.
///
/// We retrieve the user’s current instance. If it’s joinable, produce a
/// `vrchat.com/home/launch` link. Otherwise produce a `.../world/<worldId>/info` link.
pub async fn handle_instance(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    _user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    // 1) Determine which VRChat account to use
    let configured_account = match ctx.bot_config_repo.get_value("vrchat_active_account").await? {
        Some(val) if !val.trim().is_empty() => val,
        _ => "broadcaster".to_string(),
    };
    info!("handle_instance => VRChat account from config: '{}'", configured_account);

    // 2) Retrieve that credential
    let all_vrc_creds = ctx.credentials_repo.list_credentials_for_platform(&Platform::VRChat).await?;
    let vrc_cred_opt = all_vrc_creds
        .into_iter()
        .find(|c| c.user_name.eq_ignore_ascii_case(&configured_account));

    let cred = match vrc_cred_opt {
        Some(c) => c,
        None => {
            return Ok(format!(
                "No VRChat credentials found for account '{}'. \
Please set 'vrchat_active_account' or run 'account add vrchat'.",
                configured_account
            ));
        }
    };

    // 3) Fetch instance
    let client = VRChatClient::new(&cred.primary_token)?;
    let inst_opt = client.fetch_current_instance_api().await?;
    let inst = match inst_opt {
        Some(i) => i,
        None => return Ok("User is offline or no instance found.".into()),
    };

    // 4) Retrieve the world name from world_id
    let world_id = inst.world_id.clone().unwrap_or_default();
    if world_id.is_empty() {
        return Ok("Currently in an unknown/hidden world.".to_string());
    }
    let winfo = client.fetch_world_info(&world_id).await?;
    let world_name = winfo.name;

    // 5) Decide if instance is joinable
    //    e.g. if location doesn’t have "private"/"invite", treat it as joinable
    let location = inst.location.unwrap_or_default().to_lowercase();
    let instance_id = inst.instance_id.unwrap_or_default();
    if instance_id.is_empty() {
        return Ok(format!("Currently in world '{}', unknown instance.", world_name));
    }
    let is_joinable = !(location.contains("private") || location.contains("invite"));

    // 6) Construct link
    let link = if is_joinable {
        format!(
            "https://vrchat.com/home/launch?worldId={}&instanceId={}",
            world_id, instance_id
        )
    } else {
        format!("https://vrchat.com/home/world/{}/info", world_id)
    };

    Ok(format!(
        "[world] '{}' - link: {}",
        world_name, link
    ))
}

/// handle_vrchat_online_offline might handle sub-commands if needed (example).
pub async fn handle_vrchat_online_offline(
    _cmd: &Command,
    _ctx: &CommandContext<'_>,
    _user: &User,
    raw_args: &str,
) -> Result<String, Error> {
    let arg = raw_args.trim().to_lowercase();
    match arg.as_str() {
        "offline" => Ok("VRChat commands are now forced offline. (Stub)".to_string()),
        "online" => Ok("VRChat commands now assume online. (Stub)".to_string()),
        _ => {
            warn!("!vrchat unknown argument => '{}'", raw_args);
            Ok("Usage: !vrchat <offline|online>".to_string())
        }
    }
}