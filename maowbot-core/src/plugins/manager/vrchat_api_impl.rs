// File: maowbot-core/src/plugins/manager/vrchat_api_impl.rs
//! plugins/manager/vrchat_api_impl.rs
//!
//! Implements VrchatApi for PluginManager (get_current_world, etc.).

use crate::Error;
use maowbot_common::models::platform::Platform;
use crate::repositories::postgres::user::UserRepo;
use crate::platforms::vrchat::client::VRChatClient;
use maowbot_common::traits::api::{
    VrchatApi, VRChatWorldBasic, VRChatAvatarBasic, VRChatInstanceBasic
};
use crate::plugins::manager::core::PluginManager;
use async_trait::async_trait;
use maowbot_common::traits::api::CredentialsApi;
use std::path::PathBuf;

#[async_trait]
impl VrchatApi for PluginManager {
    async fn vrchat_get_current_world(&self, account_name: &str) -> Result<VRChatWorldBasic, Error> {
        let name_to_use = if account_name.is_empty() {
            let all_creds = self.list_credentials(Some(Platform::VRChat)).await?;
            if all_creds.is_empty() {
                return Err(Error::Platform("No VRChat credentials in DB.".into()));
            }
            if all_creds.len() > 1 {
                return Err(Error::Platform(
                    "Multiple VRChat accounts found. Please specify an account name.".into()
                ));
            }
            let c = &all_creds[0];
            let user_opt = self.user_repo.get(c.user_id).await?;
            user_opt
                .and_then(|u| u.global_username)
                .unwrap_or_else(|| c.user_id.to_string())
        } else {
            account_name.to_string()
        };

        let user = self.user_repo
            .get_by_global_username(&name_to_use)
            .await?
            .ok_or_else(|| Error::Platform(format!("No user found w/ name='{}'", name_to_use)))?;

        let cred_opt = {
            if let Some(am) = &self.auth_manager {
                let lock = am.lock().await;
                lock.credentials_repo.get_credentials(&Platform::VRChat, user.user_id).await?
            } else {
                return Err(Error::Auth("No auth manager set".into()));
            }
        };
        let cred = cred_opt.ok_or_else(|| Error::Platform(
            format!("No VRChat credential for user='{}'", name_to_use)
        ))?;

        let client = VRChatClient::new(&cred.primary_token)?;
        let maybe_world = client.fetch_current_world_api().await?;
        let w = match maybe_world {
            Some(w) => w,
            None => {
                return Err(Error::Platform("User is offline or not in any world.".into()));
            }
        };

        Ok(VRChatWorldBasic {
            name: w.name,
            author_name: w.author_name,
            updated_at: w.updated_at.unwrap_or_default(),
            created_at: w.published_at.unwrap_or_default(),
            capacity: w.capacity,
            release_status: w.release_status.unwrap_or_else(|| "unknown".to_string()),
            description: w.description.unwrap_or_default(),
        })
    }

    async fn vrchat_get_current_avatar(&self, account_name: &str) -> Result<VRChatAvatarBasic, Error> {
        let name_to_use = account_name.to_string();
        let user = self.user_repo
            .get_by_global_username(&name_to_use)
            .await?
            .ok_or_else(|| Error::Platform(format!("No user found w/ name='{}'", name_to_use)))?;

        let cred_opt = {
            if let Some(am) = &self.auth_manager {
                let lock = am.lock().await;
                lock.credentials_repo.get_credentials(&Platform::VRChat, user.user_id).await?
            } else {
                return Err(Error::Auth("No auth manager set".into()));
            }
        };
        let cred = cred_opt.ok_or_else(|| Error::Platform("No VRChat credential".into()))?;
        let client = VRChatClient::new(&cred.primary_token)?;
        let av_opt = client.fetch_current_avatar_api().await?;
        match av_opt {
            Some(av) => Ok(VRChatAvatarBasic {
                avatar_id: av.avatar_id,
                avatar_name: av.name,
            }),
            None => Err(Error::Platform("Offline or no current avatar.".into())),
        }
    }

    async fn vrchat_change_avatar(&self, account_name: &str, new_avatar_id: &str) -> Result<(), Error> {
        let user = self.user_repo
            .get_by_global_username(account_name)
            .await?
            .ok_or_else(|| Error::Platform(format!("No user found for '{}'", account_name)))?;

        let cred_opt = {
            if let Some(am) = &self.auth_manager {
                let lock = am.lock().await;
                lock.credentials_repo.get_credentials(&Platform::VRChat, user.user_id).await?
            } else {
                return Err(Error::Auth("No auth manager set".into()));
            }
        };
        let cred = cred_opt.ok_or_else(|| Error::Platform("No VRChat credential".into()))?;
        let client = VRChatClient::new(&cred.primary_token)?;
        client.select_avatar(new_avatar_id).await?;
        Ok(())
    }

    async fn vrchat_get_current_instance(&self, account_name: &str) -> Result<VRChatInstanceBasic, Error> {
        let name_to_use = if account_name.is_empty() {
            let all_creds = self.list_credentials(Some(Platform::VRChat)).await?;
            if all_creds.is_empty() {
                return Err(Error::Platform("No VRChat credentials in DB.".into()));
            }
            if all_creds.len() > 1 {
                return Err(Error::Platform(
                    "Multiple VRChat accounts found. Please specify an account name.".into()
                ));
            }
            let c = &all_creds[0];
            let user_opt = self.user_repo.get(c.user_id).await?;
            user_opt
                .and_then(|u| u.global_username)
                .unwrap_or_else(|| c.user_id.to_string())
        } else {
            account_name.to_string()
        };

        let user = self.user_repo
            .get_by_global_username(&name_to_use)
            .await?
            .ok_or_else(|| Error::Platform(format!("No user found w/ name='{}'", name_to_use)))?;

        let cred_opt = {
            if let Some(am) = &self.auth_manager {
                let lock = am.lock().await;
                lock.credentials_repo.get_credentials(&Platform::VRChat, user.user_id).await?
            } else {
                return Err(Error::Auth("No auth manager set".into()));
            }
        };
        let cred = cred_opt.ok_or_else(|| Error::Platform(
            format!("No VRChat credential for user='{}'", name_to_use)
        ))?;

        let client = VRChatClient::new(&cred.primary_token)?;
        let inst_opt = client.fetch_current_instance_api().await?;

        let inst = match inst_opt {
            Some(i) => i,
            None => return Err(Error::Platform("User is offline or has no instance.".into())),
        };

        Ok(VRChatInstanceBasic {
            world_id: inst.world_id,
            instance_id: inst.instance_id,
            location: inst.location,
        })
    }
}

// ----------------------------------------------------
// NEW METHOD: vrchat_get_local_storage_path (example)
// ----------------------------------------------------
// We add a helper that attempts to read a path from "additional_data" in the VRChat credential.
// If not found, we guess a Windows path. Modify as needed for your environment.
impl PluginManager {
    /// Returns the local Avatars folder path for the given VRChat user, if known.
    /// Typically: `C:\Users\<USERNAME>\AppData\LocalLow\VRChat\VRChat\OSC\usr_...\Avatars`
    pub async fn vrchat_get_local_storage_path(&self, account_name: &str) -> Result<PathBuf, Error> {
        use std::env;

        let user = self.user_repo
            .get_by_global_username(account_name)
            .await?
            .ok_or_else(|| Error::Platform(format!("No user found for '{}'", account_name)))?;

        let cred_opt = {
            if let Some(am) = &self.auth_manager {
                let lock = am.lock().await;
                lock.credentials_repo.get_credentials(&Platform::VRChat, user.user_id).await?
            } else {
                return Err(Error::Auth("No auth manager set".into()));
            }
        };
        let cred = cred_opt.ok_or_else(|| Error::Platform("No VRChat credential found".into()))?;

        // If `additional_data` has a "vrc_local_storage_path" key, use it.
        if let Some(ref data) = cred.additional_data {
            if let Some(path_val) = data.get("vrc_local_storage_path") {
                if let Some(path_str) = path_val.as_str() {
                    let p = PathBuf::from(path_str);
                    if p.exists() {
                        return Ok(p);
                    }
                }
            }
        }

        // Otherwise, fallback to a naive guess for Windows
        // This can be replaced with cross-platform logic, or a config file, etc.
        let home_dir = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".to_string());
        // The user_id might be "usr_abc123..."
        let user_platform_id = cred.platform_id.clone().unwrap_or_else(|| "usr_XXXX".to_string());
        let sub = format!("AppData\\LocalLow\\VRChat\\VRChat\\OSC\\{}\\Avatars", user_platform_id);
        let guessed = PathBuf::from(home_dir).join(sub);
        Ok(guessed)
    }
}
