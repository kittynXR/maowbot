use chrono::{Duration};
use tracing::{info, error};
use maowbot_common::traits::repository_traits::CredentialsRepository;
use crate::auth::manager::AuthManager;
use crate::Error;
use crate::repositories::postgres::credentials::{PostgresCredentialsRepository};


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
        let user_id = cred.user_id;

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

/// Refreshes **all** credentials in the database that have a valid `refresh_token`.
/// This ensures both “bot” credentials (`is_bot = true`) and user “account” credentials
/// (`is_bot = false`) are attempted. If the refresh fails, it logs the error but continues.
///
/// This also covers the scenario where we simply want to refresh everything at startup.
pub async fn refresh_all_refreshable_credentials(
    creds_repo: &PostgresCredentialsRepository,
    auth_manager: &mut AuthManager,
) -> Result<(), Error> {
    let all_creds = creds_repo.get_all_credentials().await?;
    let mut refreshable = Vec::new();

    // Gather only those that have a refresh_token (indicating they're refreshable)
    for cred in &all_creds {
        if cred.refresh_token.is_some() {
            refreshable.push(cred.clone());
        }
    }

    if refreshable.is_empty() {
        info!("No refreshable credentials found in the database.");
        return Ok(());
    }

    info!(
        "Found {} refreshable credential(s). Attempting to refresh all...",
        refreshable.len()
    );

    for cred in refreshable {
        let platform = cred.platform.clone();
        let user_id = cred.user_id;
        match auth_manager.refresh_platform_credentials(&platform, &user_id).await {
            Ok(updated_cred) => {
                info!(
                    "Refreshed credential for platform={:?}, user_id={}. \
                     New expires_at={:?}",
                    updated_cred.platform,
                    updated_cred.user_id,
                    updated_cred.expires_at
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