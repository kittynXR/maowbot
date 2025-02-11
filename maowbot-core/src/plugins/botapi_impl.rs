// File: maowbot-core/src/plugins/botapi_impl.rs

use std::collections::HashMap;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

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

    // --------------------------------------------------------------------------------
    //  All user-related methods now pass Uuid for user_id:
    // --------------------------------------------------------------------------------

    async fn create_user(&self, new_user_id: Uuid, display_name: &str) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        let user = crate::models::User {
            user_id: new_user_id,
            global_username: Some(display_name.to_string()),
            created_at: chrono::Utc::now(),
            last_seen: chrono::Utc::now(),
            is_active: true,
        };
        user_repo.create(&user).await?;
        Ok(())
    }

    async fn remove_user(&self, user_id: Uuid) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        user_repo.delete(user_id).await?;
        Ok(())
    }

    async fn get_user(&self, user_id: Uuid) -> Result<Option<crate::models::User>, Error> {
        let user_repo = self.user_repo.clone();
        let found = user_repo.get(user_id).await?;
        Ok(found)
    }

    async fn update_user_active(&self, user_id: Uuid, is_active: bool) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        if let Some(mut u) = user_repo.get(user_id).await? {
            u.is_active = is_active;
            u.last_seen = chrono::Utc::now();
            user_repo.update(&u).await?;
        }
        Ok(())
    }

    /// We still do naive substring matching for search_users. This remains a string-based query
    /// so no change to the signature.
    async fn search_users(&self, query: &str) -> Result<Vec<crate::models::User>, Error> {
        let user_repo = self.user_repo.clone();
        let all_users = user_repo.list_all().await?;
        let filtered = all_users.into_iter()
            .filter(|u| {
                // Convert user_id to string or do something else if you want
                let user_id_str = u.user_id.to_string();
                user_id_str.contains(query)
                    || u.global_username.as_deref().unwrap_or("").contains(query)
            })
            .collect();
        Ok(filtered)
    }

    async fn find_user_by_name(&self, name: &str) -> Result<User, Error> {
        // 1) call search_users
        let all = self.search_users(name).await?;
        // 2) find exact or partial match
        if all.is_empty() {
            Err(Error::Auth(format!("No user with name='{}'", name)))
        } else if all.len() > 1 {
            // optional: return error or pick the first
            Err(Error::Auth(format!("Multiple matches for '{}'", name)))
        } else {
            // exactly 1
            Ok(all[0].clone())
        }
    }

    // --------------------------------------------------------------------------------
    //  OAuth flows
    // --------------------------------------------------------------------------------

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
            .expect("auth_manager is None!")
            .lock()
            .await;

        authmgr
            .complete_auth_flow_for_user_multi(platform, &user_id, keys)
            .await
    }

    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.revoke_credentials(&platform, &user_id.to_string()).await
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
            pc_repo.delete_platform_config(platform_config_id.parse().unwrap()).await?;
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
            .expect("No auth manager set in plugin manager");
        let mut auth_manager_locked = auth_mgr_arc.lock().await;
        auth_manager_locked.bot_config_repo.get_value(key).await
    }

    async fn set_bot_config_value(&self, key: &str, value: &str) -> Result<(), Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .expect("No auth manager set in plugin manager");
        let mut auth_manager_locked = auth_mgr_arc.lock().await;
        auth_manager_locked.bot_config_repo.set_value(key, value).await
    }

    async fn subscribe_chat_events(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent> {
        // If we have an event bus, subscribe to it. If not, return a dummy empty receiver.
        if let Some(bus) = &self.event_bus {
            bus.subscribe(buffer_size).await
        } else {
            let (_tx, rx) = mpsc::channel(1);
            rx
        }
    }
}