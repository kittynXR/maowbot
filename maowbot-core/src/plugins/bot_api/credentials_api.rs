//! plugins/bot_api/credentials_api.rs
//!
//! Sub-trait for credentials flows (OAuth, 2FA, API keys, etc.).

use crate::{Error, models::{Platform, PlatformCredential}};
use async_trait::async_trait;
use uuid::Uuid;
use std::collections::HashMap;

/// Credential-related trait: beginning and completing auth flows, storing tokens, etc.
#[async_trait]
pub trait CredentialsApi: Send + Sync {
    /// Step 1 of OAuth or similar flows: returns a URL or instructions to begin.
    async fn begin_auth_flow(&self, platform: Platform, is_bot: bool) -> Result<String, Error>;

    /// Completes the auth flow with a code (no user_id). Typically for “bot” credentials or system credentials.
    async fn complete_auth_flow(
        &self,
        platform: Platform,
        code: String
    ) -> Result<PlatformCredential, Error>;

    /// Completes the auth flow for a specific user (by UUID).
    async fn complete_auth_flow_for_user(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;

    /// Completes the flow with multiple input fields (username/password, or multiple tokens).
    async fn complete_auth_flow_for_user_multi(
        &self,
        platform: Platform,
        user_id: Uuid,
        keys: HashMap<String, String>,
    ) -> Result<PlatformCredential, Error>;

    /// Completes a 2FA step for a user (like VRChat).
    async fn complete_auth_flow_for_user_2fa(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;

    /// Revoke credentials for a user (by UUID).
    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<(), Error>;

    /// Refresh credentials for a user (by UUID).
    async fn refresh_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<PlatformCredential, Error>;

    /// List all credentials, optionally filtered by platform.
    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error>;

    /// Store (create/update) an already-built credential in the DB.
    async fn store_credential(&self, cred: PlatformCredential) -> Result<(), Error>;
}