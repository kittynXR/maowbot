// src/plugins/botapi_impl.rs

use async_trait::async_trait;
use std::sync::Arc;
use crate::plugins::manager::PluginManager;
use crate::plugins::types::PluginRecord;
use crate::plugins::plugin_connection::PluginConnection;
use crate::plugins::types::PluginType;
use crate::Error;
use crate::eventbus::{BotEvent, EventBus};
use crate::plugins::bot_api::{BotApi, StatusData, PlatformConfigData};
use crate::models::{Platform, PlatformCredential, User};
use crate::repositories::postgres::bot_config::BotConfigRepository;
use crate::repositories::postgres::credentials::CredentialsRepository;
use crate::repositories::postgres::platform_config::PlatformConfigRepository;
use crate::repositories::postgres::user::{UserRepo, UserRepository};

#[async_trait]
impl BotApi for PluginManager {
    async fn list_plugins(&self) -> Vec<String> {
        let records = self.get_plugin_records();
        records
            .into_iter()
            .map(|r| {
                let suffix = if r.enabled { "" } else { " (disabled)" };
                format!("{}{}", r.name, suffix)
            })
            .collect()
    }

    async fn status(&self) -> StatusData {
        let connected = self.list_connected_plugins().await;
        let connected_names: Vec<_> = connected
            .into_iter()
            .map(|p| {
                let suffix = if p.is_enabled { "" } else { " (disabled)" };
                format!("{}{}", p.name, suffix)
            })
            .collect();

        StatusData {
            connected_plugins: connected_names,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    async fn shutdown(&self) {
        if let Some(bus) = &self.event_bus {
            bus.shutdown();
        }
    }

    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error> {
        self.toggle_plugin_async(plugin_name, enable).await
    }

    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error> {
        self.remove_plugin(plugin_name).await
    }

    async fn create_user(&self, new_user_id: &str, display_name: &str) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        let user = crate::models::User {
            user_id: new_user_id.to_string(),
            global_username: Some(display_name.to_string()),
            created_at: chrono::Utc::now(),
            last_seen: chrono::Utc::now(),
            is_active: true,
        };
        user_repo.create(&user).await?;
        Ok(())
    }

    async fn remove_user(&self, user_id: &str) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        user_repo.delete(user_id).await?;
        Ok(())
    }

    async fn get_user(&self, user_id: &str) -> Result<Option<crate::models::User>, Error> {
        let user_repo = self.user_repo.clone();
        let found = user_repo.get(user_id).await?;
        Ok(found)
    }

    async fn update_user_active(&self, user_id: &str, is_active: bool) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        if let Some(mut u) = user_repo.get(user_id).await? {
            u.is_active = is_active;
            u.last_seen = chrono::Utc::now();
            user_repo.update(&u).await?;
        }
        Ok(())
    }

    async fn search_users(&self, query: &str) -> Result<Vec<crate::models::User>, Error> {
        let user_repo = self.user_repo.clone();
        let all_users = user_repo.list_all().await?;
        let filtered = all_users.into_iter()
            .filter(|u| {
                u.user_id.contains(query)
                    || u.global_username.as_deref().unwrap_or("").contains(query)
            })
            .collect();
        Ok(filtered)
    }

    async fn begin_auth_flow(
        &self,
        platform: Platform,
        is_bot: bool
    ) -> Result<String, Error> {
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
            lock.complete_auth_flow(platform, code).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn complete_auth_flow_for_user(
        &self,
        platform: Platform,
        code: String,
        user_id: &str
    ) -> Result<PlatformCredential, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.complete_auth_flow_for_user(platform, code, user_id).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: &str
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.revoke_credentials(&platform, user_id).await
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

    async fn create_platform_config(
        &self,
        platform: Platform,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
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
            lock.count_platform_configs_for(&platform_str).await
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

    async fn remove_platform_config(&self, platform_config_id: &str) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            let pc_repo = &lock.platform_config_repo;
            pc_repo.delete_platform_config(platform_config_id).await?;
            Ok(())
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }
}