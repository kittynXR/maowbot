use chrono::{Utc};
use tracing::info;
use crate::Error;
use crate::models::{Command, User, Platform};
use crate::services::command_service::CommandContext;
use crate::platforms::twitch::client::TwitchHelixClient;

/// The `handle_followage` function implements the `!followage` command.
/// It tries to determine how long the user has been following the configured broadcaster channel.
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
    // 1) Fetch the broadcaster channel name from bot_config.
    //    The TUI command `ttv broadcaster <channel>` stores it in `ttv_broadcaster_channel`.
    //
    let broadcaster_chan_opt = ctx.bot_config_repo.get_value("ttv_broadcaster_channel").await?;
    let broadcaster_chan = match broadcaster_chan_opt {
        Some(val) if !val.trim().is_empty() => val,
        _ => {
            // If it's missing or empty, we cannot proceed:
            return Ok(
                "No broadcaster channel is set in bot_config (key='ttv_broadcaster_channel'). \
Use `ttv broadcaster <channel>` in the TUI first."
                    .to_string()
            );
        }
    };

    //
    // 2) Strip off any leading '#' from that stored channel name, so we get the raw user name.
    //
    let raw_broadcaster_name = broadcaster_chan.trim().trim_start_matches('#');

    if raw_broadcaster_name.is_empty() {
        return Ok(format!(
            "Bot config says broadcaster channel is '{}', but that does not look valid.",
            broadcaster_chan
        ));
    }

    //
    // 3) Look up a Twitch credential for `raw_broadcaster_name`:
    //
    let all_twitch_creds = ctx
        .credentials_repo
        .list_credentials_for_platform(&Platform::Twitch)
        .await?;

    let broadcaster_cred_opt = all_twitch_creds.into_iter().find(|c| {
        c.user_name.eq_ignore_ascii_case(raw_broadcaster_name)
    });

    let broadcaster_cred = match broadcaster_cred_opt {
        Some(c) => c,
        None => {
            return Ok(format!(
                "No Twitch credentials found for broadcaster account '{}'. \
Please add a credential for that user (or ensure user_name matches '{}').",
                raw_broadcaster_name, raw_broadcaster_name
            ));
        }
    };

    let broadcaster_id = broadcaster_cred
        .platform_id
        .clone()
        .unwrap_or_default();
    if broadcaster_id.is_empty() {
        return Ok(format!(
            "The broadcaster credential for '{}' has no .platform_id set (external Twitch user ID). \
Cannot fetch follow data.",
            raw_broadcaster_name
        ));
    }

    //
    // 4) Get the viewer’s Twitch user_id from platform_identity.
    //    If user doesn't have a Twitch identity, we can't check their follow.
    //
    let viewer_identity_opt = ctx
        .user_service
        .platform_identity_repo
        .get_by_user_and_platform(user.user_id, &Platform::Twitch)
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
    // 5) Build a Helix client from the broadcaster’s token and client_id
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
    // 6) Fetch the follow date
    //
    let follow_date_opt = helix_client
        .fetch_follow_date(&viewer_id, &broadcaster_id)
        .await?;

    let follow_date = match follow_date_opt {
        Some(fd) => fd,
        None => {
            return Ok(format!(
                "{} is not following channel '{}' (or the data is unavailable).",
                user_name, broadcaster_chan
            ));
        }
    };

    //
    // 7) Compute how long they've followed
    //
    let now = Utc::now();
    let diff = now.signed_duration_since(follow_date);
    let total_days = diff.num_days();
    let months = total_days / 30; // approximate
    let leftover_days = total_days % 30;

    if total_days < 1 {
        let hours = diff.num_hours();
        return Ok(format!(
            "{} has been following {} for {} hour(s).",
            user_name, broadcaster_chan, hours
        ));
    }

    if months < 1 {
        // less than 1 month
        return Ok(format!(
            "{} has been following {} for {} day(s).",
            user_name, broadcaster_chan, total_days
        ));
    }

    // If there's at least 1 month, show months + leftover days
    Ok(format!(
        "{} has been following {} for {} month(s) and {} day(s).",
        user_name, broadcaster_chan, months, leftover_days
    ))
}