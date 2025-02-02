//! src/tasks/credential_refresh.rs

use chrono::{Duration};
use tracing::{info, error};
use crate::repositories::postgres::PostgresCredentialsRepository;
use crate::auth::AuthManager;
// use crate::models::Platform;
use crate::Error;
use crate::repositories::postgres::credentials::CredentialsRepository;

/// Checks for credentials that will expire within `within_minutes` from now.
/// For each such credential, calls `AuthManager::refresh_platform_credentials`.
///
/// Returns Ok(()) even if some credentials fail to refresh (logs errors).
pub async fn refresh_expiring_tokens(
    creds_repo: &PostgresCredentialsRepository,
    auth_manager: &mut AuthManager,
    within_minutes: i64,
) -> Result<(), Error> {
    let duration = Duration::minutes(within_minutes);
    let expiring = creds_repo.get_expiring_credentials(duration).await?;

    if expiring.is_empty() {
        info!("No credentials expiring in the next {} minutes.", within_minutes);
        return Ok(());
    }

    info!("Found {} credential(s) expiring soon; attempting to refresh...", expiring.len());

    // For each credential, attempt to refresh
    for cred in expiring {
        let platform = cred.platform.clone();
        let user_id = cred.user_id.clone();

        // Because refresh_platform_credentials requires (platform, user_id)
        // and it returns a new credential on success:
        match auth_manager.refresh_platform_credentials(&platform, &user_id).await {
            Ok(new_cred) => {
                info!(
                    "Successfully refreshed credential for platform={:?}, user_id={}",
                    new_cred.platform, new_cred.user_id
                );
            }
            Err(e) => {
                error!(
                    "Failed to refresh credential for platform={:?}, user_id={}: {:?}",
                    platform, user_id, e
                );
            }
        }
    }

    Ok(())
}
