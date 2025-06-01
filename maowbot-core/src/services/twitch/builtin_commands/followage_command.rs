use chrono::Utc;
use tracing::info;
use maowbot_common::models::Command;
use maowbot_common::models::platform::{Platform};
use maowbot_common::models::user::User;
use crate::Error;
use crate::platforms::twitch::client::TwitchHelixClient;
use crate::services::twitch::command_service::CommandContext;

/// The `handle_followage` function implements the `!followage` command.
/// It now locates the broadcaster’s Twitch Helix credential by querying
/// `ctx.credentials_repo.get_broadcaster_credential(Platform::Twitch)`,
/// rather than reading from `bot_config`.
pub async fn handle_followage(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    _raw_args: &str,
) -> Result<String, Error> {
    let user_name = user
        .global_username
        .as_deref()
        .unwrap_or("<unknown user>");

    info!(
        "Handling followage for user_id={} (username='{}') in channel='{}'",
        user.user_id,
        user_name,
        ctx.channel
    );

    //
    // 1) Retrieve the broadcaster’s Twitch Helix credential from the repository.
    //
    let broadcaster_cred_opt = ctx.credentials_repo
        .get_broadcaster_credential(&Platform::Twitch)
        .await?;

    let broadcaster_cred = match broadcaster_cred_opt {
        Some(cred) => cred,
        None => {
            return Ok(
                "No broadcaster credential found for Twitch. \
Please designate an is_broadcaster Twitch Helix account first."
                    .to_string()
            );
        }
    };

    //
    // 2) Make sure the broadcaster credential has the external Twitch user ID in `platform_id`.
    //
    let broadcaster_id = match broadcaster_cred.platform_id.clone() {
        Some(pid) if !pid.trim().is_empty() => pid,
        _ => {
            return Ok(format!(
                "Broadcaster credential for user_name='{}' has no .platform_id. \
Cannot fetch follow info.",
                broadcaster_cred.user_name
            ));
        }
    };

    //
    // 3) Look up the viewer’s Twitch identity (Helix ID). If the user doesn’t have a Twitch
    //    platform identity linked, we cannot check follow status.
    //
    let viewer_identity_opt = ctx
        .user_service
        .platform_identity_repo
        .get_by_user_and_platform(user.user_id, &Platform::TwitchIRC)
        .await?;

    let viewer_id = match viewer_identity_opt {
        Some(ident) => ident.platform_user_id.clone(),
        None => {
            return Ok(format!(
                "You have not linked any Twitch ID, {}. I cannot check your follow status.",
                user_name
            ));
        }
    };

    //
    // 4) Build a Helix client from the broadcaster’s credential (token + client_id).
    //
    let bearer_token = &broadcaster_cred.primary_token;
    let client_id_str = match &broadcaster_cred.additional_data {
        Some(json) => {
            if let Some(c_id) = json.get("client_id").and_then(|v| v.as_str()) {
                c_id.to_string()
            } else if let Some(vc_id) = json.get("validate_client_id").and_then(|v| v.as_str()) {
                vc_id.to_string()
            } else {
                "MISSING_CLIENT_ID".to_string()
            }
        }
        None => "MISSING_CLIENT_ID".to_string(),
    };

    let helix_client = TwitchHelixClient::new(bearer_token, &client_id_str);

    //
    // 5) Fetch the follow date using Helix: (viewer_id, broadcaster_id).
    //
    let follow_date_opt = helix_client
        .fetch_follow_date(&viewer_id, &broadcaster_id)
        .await?;

    let follow_date = match follow_date_opt {
        Some(fd) => fd,
        None => {
            return Ok(format!(
                "{} is not following that channel (or data is unavailable).",
                user_name
            ));
        }
    };

    //
    // 6) Compute how long they’ve been following
    //
    let now = Utc::now();
    let diff = now.signed_duration_since(follow_date);
    let total_days = diff.num_days();
    let months = total_days / 30; // approximate
    let leftover_days = total_days % 30;

    // Shorter cases for <1 day or <1 month
    if total_days < 1 {
        let hours = diff.num_hours();
        return Ok(format!(
            "{} has been following for about {} hour(s).",
            user_name, hours
        ));
    }
    if months < 1 {
        return Ok(format!(
            "{} has been following for {} day(s).",
            user_name, total_days
        ));
    }

    // If >=1 month, show months + leftover days
    Ok(format!(
        "{} has been following for {} month(s) and {} day(s).",
        user_name, months, leftover_days
    ))
}
