//! Built-in `!vanish` command, now calling our centralized TwitchApi.
//!
//! Usage: `!vanish [yes|y|true|1]`
//! • Always issues a 1s timeout (purge) on the caller.
//! • If a truthy flag is present, sends a “🪄 … has vanished!” confirmation.

use crate::Error;
use crate::services::twitch::command_service::CommandContext;
use maowbot_common::models::{Command, user::User};
use maowbot_common::models::platform::Platform::TwitchIRC;
use maowbot_common::traits::repository_traits::CredentialsRepository;
use maowbot_common::traits::api::TwitchApi;

/// Returns true for “yes, y, true, 1”
fn is_truthy(flag: &str) -> bool {
    matches!(flag.trim().to_ascii_lowercase().as_str(), "yes" | "y" | "true" | "1")
}

pub async fn handle_vanish(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    raw_args: &str,
) -> Result<String, Error> {
    // 1) Resolve target login
    let mut login = user.global_username.clone().unwrap_or_else(String::new);
    if login.is_empty() {
        if let Ok(Some(cred)) = ctx
            .credentials_repo
            .get_credentials(&TwitchIRC, user.user_id)
            .await
        {
            login = cred.user_name.clone();
        }
    }
    if login.is_empty() {
        login = user.user_id.to_string();
    }

    // 2) Invoke our new timeout API
    if let Some(api) = &ctx.plugin_manager {
        // pick which bot account to send from:
        let sender = ctx
            .respond_credential_name
            .as_deref()
            .unwrap_or_else(|| ctx.channel.strip_prefix('#').unwrap_or(ctx.channel));
        api.timeout_twitch_user(sender, ctx.channel, &login, 1, None)
            .await?;
    }

    // 3) Optionally confirm
    if is_truthy(raw_args) {
        Ok(format!("🪄 {} has vanished!", login))
    } else {
        Ok(String::new())
    }
}
