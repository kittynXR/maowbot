use crate::Error;
use crate::models::{Command, User};
use crate::services::command_service::CommandContext;
use tracing::{info, warn};

use crate::platforms::vrchat::client::VRChatClient;
use crate::models::Platform;

/// handle_world is invoked for the `!world` command.
pub async fn handle_world(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    // Instead of using respond_credential_name, we read which VRChat account to use from bot_config
    let configured_account = match ctx.bot_config_repo.get_value("vrchat_active_account").await? {
        Some(val) if !val.trim().is_empty() => val,
        _ => "broadcaster".to_string(), // fallback default
    };
    info!("handle_world => VRChat account from config: '{}'", configured_account);

    // Now we see if that account actually has VRChat credentials. We'll do this:
    let all_vrc_creds = ctx.credentials_repo.list_credentials_for_platform(&Platform::VRChat).await?;
    // We'll match if user_name == configured_account (case-insensitive), or the user’s global_username
    let vrc_cred_opt = all_vrc_creds.into_iter().find(|c| {
        c.user_name.eq_ignore_ascii_case(&configured_account)
    });

    let cred = match vrc_cred_opt {
        Some(c) => c,
        None => {
            return Ok(format!(
                "No VRChat credentials found for account '{}'. Please check 'vrchat account' or 'account add vrchat'.",
                configured_account
            ));
        }
    };

    let client = VRChatClient::new(&cred.primary_token)?;
    let winfo_opt = client.fetch_current_world_api().await?;
    if winfo_opt.is_none() {
        return Ok("User is offline or not in any world.".to_string());
    }
    let w = winfo_opt.unwrap();

    // We'll break the output into multiple <SPLIT> segments
    let part1 = format!("[World Info]\nName: {}", w.name);
    let part2 = format!(
        "Author: {}\nCapacity: {}\nStatus: {}",
        w.author_name,
        w.capacity,
        w.release_status.clone().unwrap_or_default(),
    );
    let pub_str = w.published_at.clone().unwrap_or("(unknown)".to_string());
    let upd_str = w.updated_at.clone().unwrap_or("(unknown)".to_string());
    let part3 = format!("Published: {}\nUpdated: {}", pub_str, upd_str);

    let mut part4 = String::new();
    if let Some(desc) = w.description {
        if !desc.trim().is_empty() {
            let snippet = if desc.len() > 300 {
                let d = &desc[..300];
                format!("{}\n(…truncated…)", d)
            } else {
                desc
            };
            part4 = format!("Description:\n{}", snippet);
        }
    }

    if part4.is_empty() {
        Ok(format!("{}\n<SPLIT>{}\n<SPLIT>{}", part1, part2, part3))
    } else {
        Ok(format!("{}\n<SPLIT>{}\n<SPLIT>{}\n<SPLIT>{}", part1, part2, part3, part4))
    }
}

/// handle_instance is invoked for the `!instance` command.
pub async fn handle_instance(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    let configured_account = match ctx.bot_config_repo.get_value("vrchat_active_account").await? {
        Some(val) if !val.trim().is_empty() => val,
        _ => "broadcaster".to_string(),
    };
    info!("handle_instance => VRChat account from config: '{}'", configured_account);

    let all_vrc_creds = ctx.credentials_repo.list_credentials_for_platform(&Platform::VRChat).await?;
    let vrc_cred_opt = all_vrc_creds.into_iter().find(|c| {
        c.user_name.eq_ignore_ascii_case(&configured_account)
    });

    let cred = match vrc_cred_opt {
        Some(c) => c,
        None => {
            return Ok(format!(
                "No VRChat credentials found for account '{}'. Please check 'vrchat account' or 'account add vrchat'.",
                configured_account
            ));
        }
    };

    let client = VRChatClient::new(&cred.primary_token)?;
    let inst_opt = client.fetch_current_instance_api().await?;
    let inst = match inst_opt {
        Some(i) => i,
        None => return Ok("User is offline or no instance found.".into()),
    };

    let world_id = inst.world_id.clone().unwrap_or_default();
    if world_id.is_empty() {
        return Ok("No valid world found. Possibly hidden or offline?".into());
    }

    let winfo = client.fetch_world_info(&world_id).await?;
    let world_name = winfo.name;

    let location = inst.location.unwrap_or_default().to_lowercase();
    let can_join = !(location.contains("private") || location.contains("invite"));
    let instance_id = inst.instance_id.unwrap_or_default();

    if instance_id.is_empty() {
        return Ok(format!(
            "Currently in world '{}', but instance unknown.",
            world_name
        ));
    }

    if can_join {
        let join_url = format!(
            "vrchat://launch?ref=MaowBot&worldId={}&instanceId={}",
            world_id, instance_id
        );
        Ok(format!("Currently in world '{}' - join link: {}", world_name, join_url))
    } else {
        Ok(format!(
            "Currently in world '{}', in a non-public instance (cannot share link).",
            world_name
        ))
    }
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
        "offline" => Ok("VRChat commands are now forced offline (stub).".to_string()),
        "online" => Ok("VRChat commands now restricted to online (stub).".to_string()),
        _ => {
            warn!("!vrchat unknown argument => '{}'", raw_args);
            Ok("Usage: !vrchat <offline|online>".to_string())
        }
    }
}
