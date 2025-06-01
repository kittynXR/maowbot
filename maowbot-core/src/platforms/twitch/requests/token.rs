// Token helper – validate / refresh right before a runtime starts.
//
// Usage from a runtime:
// ```rust
// use crate::platforms::twitch::requests::token::ensure_valid_token;
// let cred = ensure_valid_token(&cred, &client_id, client_secret.as_deref(), 600).await?;
// ```

use chrono::{Duration, Utc};
use tracing::{debug, warn};

use crate::Error;
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::traits::auth_traits::PlatformAuthenticator;
use crate::platforms::twitch_irc::auth::TwitchIrcAuthenticator;
use crate::platforms::twitch_eventsub::auth::TwitchEventSubAuthenticator;

/// Ensures a credential is still valid.
/// * If `expires_at` is **> `margin_secs`** in the future, returns it unchanged.
/// * Otherwise performs an OAuth *refresh* and returns the updated credential.
///
/// `client_secret` can be **None** if you’re using a public client, but it’s
/// strongly recommended to supply it via `$TWITCH_CLIENT_SECRET`.
pub async fn ensure_valid_token(
    cred:              &PlatformCredential,
    client_id:         &str,
    client_secret:     Option<&str>,
    margin_secs:       i64,
) -> Result<PlatformCredential, Error> {
    // 1) Fast path – still plenty of time left?
    if let Some(exp) = cred.expires_at {
        if exp - Utc::now() > Duration::seconds(margin_secs) {
            return Ok(cred.clone());
        }
        warn!(
            "Twitch credential for user_id={} expires in ≤{} s – refreshing…",
            cred.user_id, margin_secs
        );
    } else {
        // No expires_at ⇒ assume refresh is possible but necessary
        warn!(
            "Twitch credential for user_id={} has no expires_at – attempting refresh…",
            cred.user_id
        );
    }

    // 2) Select the correct authenticator.
    let secret = client_secret.map(|s| s.to_string());
    let new_cred = match cred.platform {
        Platform::TwitchIRC => {
            let mut auth = TwitchIrcAuthenticator::new(client_id.to_string(), secret);
            auth.refresh(cred).await?
        }
        // Everything other than IRC we treat as “EventSub/Helix style” here.
        _ => {
            let mut auth = TwitchEventSubAuthenticator::new(client_id.to_string(), secret);
            auth.refresh(cred).await?
        }
    };

    debug!(
        "Twitch credential for user_id={} refreshed; new expiry in {} s",
        new_cred.user_id,
        new_cred
            .expires_at
            .map(|t| (t - Utc::now()).num_seconds())
            .unwrap_or_default()
    );
    Ok(new_cred)
}
