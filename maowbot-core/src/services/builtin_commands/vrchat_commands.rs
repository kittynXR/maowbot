// File: maowbot-core/src/services/builtin_commands/vrchat_commands.rs

use crate::Error;
use crate::models::{Command, User};
use crate::services::command_service::CommandContext;
use tracing::{info, warn};

use crate::platforms::vrchat::client::VRChatClient;
use crate::models::Platform;
use uuid::Uuid;
use chrono::Utc;

/// handle_world is invoked for the `!world` command.
pub async fn handle_world(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    // We want to fetch the user’s current VRChat world and display it in ~3-4 messages.
    // Because our existing pipeline only sends one final string, we will embed a special
    // delimiter <SPLIT> so the message_service can split and send multiple lines.

    let account_name = ctx
        .respond_credential_name
        .as_deref()
        .unwrap_or("");
    info!("handle_world => using VRChat account: '{}'", account_name);

    // We'll attempt to look up the VRChat credential, fetch the world, etc.
    let maybe_cred = ctx.user_service
        .platform_identity_repo
        .get_by_user_and_platform(user.user_id, &Platform::VRChat)
        .await?;

    if maybe_cred.is_none() {
        // fallback if no platform_identity row
        return Ok("You appear to have no VRChat identity on record.".to_string());
    }

    // We'll do a direct fetch using VRChatClient or we can do it more robustly
    // by re-using the plugin manager approach. For simplicity, let's do direct logic:
    let cred_opt = ctx
        .credentials_repo
        .get_credentials(&Platform::VRChat, user.user_id)
        .await?;
    let cred = match cred_opt {
        Some(c) => c,
        None => {
            return Ok("You have no VRChat session cookie or credentials stored.".to_string());
        }
    };

    let client = VRChatClient::new(&cred.primary_token)?;
    let winfo_opt = client.fetch_current_world_api().await?;
    if winfo_opt.is_none() {
        return Ok("User is offline or not in any world.".to_string());
    }
    let w = winfo_opt.unwrap();

    // We want to break into multiple lines.
    // We'll do 1) world name
    //     2) author + capacity + status
    //     3) published + updated
    //     4) optional: first 150 chars of the description
    let part1 = format!(
        "[World Info]\nName: {}",
        w.name
    );
    let part2 = format!(
        "Author: {}\nCapacity: {}\nStatus: {}",
        w.author_name,
        w.capacity,
        w.release_status.as_deref().unwrap_or("unknown"),
    );
    let pub_str = w.published_at.as_deref().unwrap_or("(unknown)");
    let upd_str = w.updated_at.as_deref().unwrap_or("(unknown)");
    let part3 = format!(
        "Published: {}\nUpdated: {}",
        pub_str, upd_str
    );

    // Possibly handle a description
    let mut part4 = String::new();
    if let Some(desc) = w.description.as_ref() {
        if !desc.trim().is_empty() {
            let snippet = if desc.len() > 300 {
                // we only show first ~300 chars for the sake of multi-message
                let d = &desc[..300];
                format!("{}\n(…truncated…)", d)
            } else {
                desc.to_string()
            };
            part4 = format!("Description:\n{}", snippet);
        }
    }

    // Combine them into multiple <SPLIT> segments.
    if part4.is_empty() {
        Ok(format!("{}\n<SPLIT>{}\n<SPLIT>{}",
                   part1, part2, part3))
    } else {
        Ok(format!("{}\n<SPLIT>{}\n<SPLIT>{}\n<SPLIT>{}",
                   part1, part2, part3, part4))
    }
}

/// handle_instance is invoked for the `!instance` command.
pub async fn handle_instance(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    // "the !instance command should just include the world name and a link to our
    //  current instance if we're joinable (not invite / invite+)."

    let cred_opt = ctx
        .credentials_repo
        .get_credentials(&Platform::VRChat, user.user_id)
        .await?;
    let cred = match cred_opt {
        Some(c) => c,
        None => return Ok("No VRChat credentials found for this user.".into()),
    };
    let client = VRChatClient::new(&cred.primary_token)?;

    let inst_opt = client.fetch_current_instance_api().await?;
    let inst = match inst_opt {
        Some(i) => i,
        None => return Ok("User is offline or no instance found.".into()),
    };
    // If we have a world_id, let's fetch the name of the world
    let world_id = inst.world_id.clone().unwrap_or_default();
    if world_id.is_empty() {
        return Ok("No valid world found. Possibly hidden or offline?".into());
    }
    let winfo = client.fetch_world_info(&world_id).await?;
    let world_name = winfo.name;

    // Check if we can share a join link
    let location = inst.location.unwrap_or_default();
    // e.g. "wrld_xxx:1234~private" or "wrld_xxx:1234~friends"
    let location_lower = location.to_lowercase();
    let can_join = !(location_lower.contains("private") || location_lower.contains("invite"));
    let instance_id = inst.instance_id.unwrap_or_default();

    if instance_id.is_empty() {
        return Ok(format!(
            "Currently in world '{}', but instance unknown.",
            world_name
        ));
    }

    if can_join {
        // example "vrchat://launch?ref=VRCDN&worldId=wrld_XXXX&instanceId=XXXX~public"
        let join_url = format!("vrchat://launch?ref=MaowBot&worldId={}&instanceId={}", world_id, instance_id);
        Ok(format!(
            "Currently in world '{}' - join link: {}",
            world_name, join_url
        ))
    } else {
        // do not share link
        Ok(format!(
            "Currently in world '{}', in a non-public instance (cannot share link).",
            world_name
        ))
    }
}

/// handle_vrchat_online_offline might handle sub-commands if needed.
pub async fn handle_vrchat_online_offline(
    _cmd: &Command,
    _ctx: &CommandContext<'_>,
    _user: &User,
    raw_args: &str,
) -> Result<String, Error> {
    let arg = raw_args.trim().to_lowercase();
    match arg.as_str() {
        "offline" => Ok("VRChat commands are now forced offline (stub).".to_string()),
        "online" => Ok("VRChat commands now restricted to online (stub).".to_string()),
        _ => {
            warn!("!vrchat unknown argument => '{}'", raw_args);
            Ok("Usage: !vrchat <offline|online>".to_string())
        }
    }
}
