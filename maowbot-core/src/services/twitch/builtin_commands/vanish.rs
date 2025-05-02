//! !vanish ‑– immediately purges the caller’s messages by issuing a 1‑second timeout.
//!
//! Usage:  !vanish [true|false]
//!   └─ optional flag         → if truthy, send a little “poof” line after timing out.
//!
//! Behaviour
//! ---------
//! • Sends “/timeout <user> 1” – Twitch clears the user’s chat history for the session.
//! • If the flag is truthy ( yes | y | true | 1 ), an extra chat line is sent
//!   after the timeout so the user sees a confirmation.
//!
//! There is intentionally **no** internal cooldown – that’s left 0 in the DB.

use crate::Error;
use crate::services::twitch::command_service::CommandContext;
use maowbot_common::models::{Command, user::User};
use maowbot_common::models::platform::Platform::TwitchIRC;

/// Truthy test for the optional flag (`true`, `1`, `yes`, `y`)
fn is_truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "y")
}

/// Built‑in handler.
pub async fn handle_vanish(
    _cmd: &Command,
    ctx: &CommandContext<'_>,
    user: &User,
    raw_args: &str,
) -> Result<String, Error> {

    //----------------------------------
    // 1.  Figure out the caller’s login
    //----------------------------------
    let mut login = user
        .global_username
        .clone()
        .unwrap_or_default();

    // If we don’t have a stored global_username, fall back to the Twitch credential.
    if login.is_empty() {
        if let Ok(Some(cred)) =
            ctx.credentials_repo
                .get_credentials(&TwitchIRC, user.user_id)
                .await
        {
            login = cred.user_name.clone();
        }
    }

    // Ultimate fallback – shouldn’t happen, but keeps the command functional.
    if login.is_empty() {
        login = user.user_id.to_string();
    }

    //----------------------------------
    // 2.  Build the response string(s)
    //----------------------------------
    let mut resp = format!("/timeout {} 1", login);

    // Optional message after the timeout.
    if is_truthy(raw_args) {
        // “<SPLIT>” lets the CommandService send two separate lines.
        resp.push_str("<SPLIT>");
        resp.push_str(&format!("🪄 {} has vanished!", login));
    }

    Ok(resp)
}
