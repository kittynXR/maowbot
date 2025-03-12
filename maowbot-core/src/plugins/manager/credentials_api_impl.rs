//! plugins/manager/credentials_api_impl.rs
//!
//! Implements CredentialsApi for PluginManager (OAuth flows, store_credential, etc.).

use std::collections::HashMap;
use async_trait::async_trait;
use uuid::Uuid;
use crate::Error;
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::traits::api::CredentialsApi;
use crate::plugins::manager::core::PluginManager;

#[async_trait]
impl CredentialsApi for PluginManager {
    async fn begin_auth_flow(&self, platform: Platform, is_bot: bool) -> Result<String, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.begin_auth_flow(platform, is_bot).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn complete_auth_flow(
        &self,
        platform: Platform,
        code: String
    ) -> Result<PlatformCredential, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.complete_auth_flow_for_user(platform, code, "00000000-0000-0000-0000-000000000000")
                .await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn complete_auth_flow_for_user(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.complete_auth_flow_for_user(platform, code, &user_id.to_string()).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn complete_auth_flow_for_user_multi(
        &self,
        platform: Platform,
        user_id: Uuid,
        keys: HashMap<String, String>,
    ) -> Result<PlatformCredential, Error> {
        let mut authmgr = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("auth_manager is None!".into()))?
            .lock()
            .await;

        authmgr.complete_auth_flow_for_user_multi(platform, &user_id, keys).await
    }

    async fn complete_auth_flow_for_user_2fa(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.complete_auth_flow_for_user_twofactor(platform, code, &user_id).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.revoke_credentials(&platform, &user_id).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn refresh_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<PlatformCredential, Error> {
        let user_uuid = match Uuid::parse_str(&user_id) {
            Ok(u) => u,
            Err(e) => return Err(Error::Auth(format!("Bad UUID: {e}"))),
        };

        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.refresh_platform_credentials(&platform, &user_uuid).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let all = lock.credentials_repo.get_all_credentials().await?;
            if let Some(p) = maybe_platform {
                Ok(all.into_iter().filter(|c| c.platform == p).collect())
            } else {
                Ok(all)
            }
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn store_credential(&self, cred: PlatformCredential) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            lock.store_credentials(&cred).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }
}