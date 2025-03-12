//! plugins/manager/platform_api_impl.rs
//!
//! Implements PlatformApi for PluginManager.

use crate::Error;
use maowbot_common::models::platform::{Platform, PlatformConfigData};
use maowbot_common::traits::api::{PlatformApi};
use crate::plugins::manager::core::PluginManager;
use async_trait::async_trait;

#[async_trait]
impl PlatformApi for PluginManager {
    async fn create_platform_config(
        &self,
        platform: Platform,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let platform_str = format!("{}", platform);
            lock.create_platform_config(&platform_str, client_id, client_secret).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn count_platform_configs_for_platform(
        &self,
        platform_str: String
    ) -> Result<usize, Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let count = lock.count_platform_configs_for(&platform_str).await?;
            Ok(count as usize)
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn list_platform_configs(
        &self,
        maybe_platform: Option<&str>
    ) -> Result<Vec<PlatformConfigData>, Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let pc_repo = &lock.platform_config_repo;
            let rows = pc_repo.list_platform_configs(maybe_platform).await?;

            let result: Vec<PlatformConfigData> = rows.into_iter().map(|r| {
                PlatformConfigData {
                    platform_config_id: r.platform_config_id,
                    platform: r.platform,
                    client_id: r.client_id,
                    client_secret: r.client_secret,
                }
            }).collect();
            Ok(result)
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn remove_platform_config(
        &self,
        platform_config_id: &str
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let pc_repo = &lock.platform_config_repo;
            pc_repo.delete_platform_config(platform_config_id.parse()?).await?;
            Ok(())
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn start_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error> {
        self.platform_manager.start_platform_runtime(platform, account_name).await
    }

    async fn stop_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error> {
        self.platform_manager.stop_platform_runtime(platform, account_name).await
    }

    async fn get_bot_config_value(&self, key: &str) -> Result<Option<String>, Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager set in plugin manager".into()))?;
        let auth_manager_locked = auth_mgr_arc.lock().await;
        auth_manager_locked.bot_config_repo.get_value(key).await
    }

    async fn set_bot_config_value(&self, key: &str, value: &str) -> Result<(), Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager set in plugin manager".into()))?;
        let auth_manager_locked = auth_mgr_arc.lock().await;
        auth_manager_locked.bot_config_repo.set_value(key, value).await
    }
}