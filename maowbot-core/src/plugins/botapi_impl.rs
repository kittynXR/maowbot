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
use crate::plugins::bot_api::{BotApi, StatusData, PlatformConfigData, AccountStatus, VRChatWorldBasic, VRChatAvatarBasic, VRChatInstanceBasic};
use crate::models::{Platform, PlatformCredential, User};
use crate::platforms::vrchat::client::{VRChatClient, VRChatWorldInfo, VRChatAvatarInfo};
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

        // Gather every stored credential
        let creds_result = self.list_credentials(None).await;
        let mut account_statuses = Vec::new();

        if let Ok(all_creds) = creds_result {
            let guard = self.platform_manager.active_runtimes.lock().await;
            for c in all_creds {
                let user_display = match self.user_repo.get(c.user_id).await {
                    Ok(Some(u)) => u
                        .global_username
                        .unwrap_or_else(|| c.user_id.to_string()),
                    _ => c.user_id.to_string(),
                };
                let platform_key = c.platform.to_string().to_lowercase();
                let user_key = c.user_id.to_string();

                let is_connected = guard.contains_key(&(platform_key.clone(), user_key));
                account_statuses.push(AccountStatus {
                    platform: platform_key,
                    account_name: user_display,
                    is_connected,
                });
            }
        }

        StatusData {
            connected_plugins: connected_names,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            account_statuses,
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

    // -----------------------------------------------------------------
    // User repository methods
    // -----------------------------------------------------------------

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

    async fn get_user(&self, user_id: Uuid) -> Result<Option<User>, Error> {
        let user_repo = self.user_repo.clone();
        Ok(user_repo.get(user_id).await?)
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

    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error> {
        let user_repo = self.user_repo.clone();
        let all_users = user_repo.list_all().await?;
        let filtered = all_users
            .into_iter()
            .filter(|u| {
                let user_id_str = u.user_id.to_string();
                user_id_str.contains(query)
                    || u.global_username.as_deref().unwrap_or("").contains(query)
            })
            .collect();
        Ok(filtered)
    }

    async fn find_user_by_name(&self, name: &str) -> Result<User, Error> {
        let all = self.search_users(name).await?;
        if all.is_empty() {
            Err(Error::Auth(format!("No user with name='{name}'")))
        } else if all.len() > 1 {
            Err(Error::Auth(format!("Multiple matches for '{name}'")))
        } else {
            Ok(all[0].clone())
        }
    }

    // -----------------------------------------------------------------
    // OAuth flows
    // -----------------------------------------------------------------

    async fn begin_auth_flow(&self, platform: Platform, is_bot: bool) -> Result<String, Error> {
        if let Some(am) = &self.auth_manager {
            let mut lock = am.lock().await;
            lock.begin_auth_flow(platform, is_bot).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn complete_auth_flow(&self, platform: Platform, code: String) -> Result<PlatformCredential, Error> {
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
            .expect("auth_manager is None!")
            .lock()
            .await;

        authmgr
            .complete_auth_flow_for_user_multi(platform, &user_id, keys)
            .await
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
            Err(Error::Auth("No auth manager set".into()))
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
            .ok_or_else(|| Error::Auth("No auth manager set in plugin manager".into()))?;
        let mut auth_manager_locked = auth_mgr_arc.lock().await;
        auth_manager_locked.bot_config_repo.get_value(key).await
    }

    async fn set_bot_config_value(&self, key: &str, value: &str) -> Result<(), Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager set in plugin manager".into()))?;
        let mut auth_manager_locked = auth_mgr_arc.lock().await;
        auth_manager_locked.bot_config_repo.set_value(key, value).await
    }

    async fn subscribe_chat_events(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent> {
        if let Some(bus) = &self.event_bus {
            bus.subscribe(buffer_size).await
        } else {
            let (_tx, rx) = mpsc::channel(1);
            rx
        }
    }

    async fn list_config(&self) -> Result<Vec<(String, String)>, Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            lock.bot_config_repo.list_all().await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        self.platform_manager.join_twitch_irc_channel(account_name, channel).await
    }

    async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error> {
        self.platform_manager.part_twitch_irc_channel(account_name, channel).await
    }

    async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), Error> {
        self.platform_manager.send_twitch_irc_message(account_name, channel, text).await
    }

    async fn store_credential(&self, cred: PlatformCredential) -> Result<(), Error> {
        if let Some(am) = &self.auth_manager {
            let lock = am.lock().await;
            lock.store_credentials(&cred).await
        } else {
            Err(Error::Auth("No auth manager set in plugin manager".into()))
        }
    }

    // -----------------------------------------------------------------
    // VRChat “world” and “avatar” lookups
    // -----------------------------------------------------------------

    async fn vrchat_get_current_world(&self, account_name: &str) -> Result<VRChatWorldBasic, Error> {
        // (unchanged from earlier) ...
        // retrieve or guess the account, then do VRChatClient::fetch_current_world_api().
        // ...
        // [omitted for brevity, assume unchanged from your code]
        #![allow(unused)] // <--- just to quiet snippet warnings

        // Example stub. Replace with your actual final from earlier:
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
        let user = self.user_repo.get_by_global_username(&name_to_use).await?
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
        match maybe_world {
            Some(w) => Ok(VRChatWorldBasic {
                name: w.name,
                author_name: w.author_name,
                updated_at: "".into(),
                created_at: "".into(),
                capacity: w.capacity,
            }),
            None => Err(Error::Platform("User is offline or not in any world.".into())),
        }
    }

    async fn vrchat_get_current_avatar(&self, account_name: &str) -> Result<VRChatAvatarBasic, Error> {
        // (unchanged from earlier) ...
        #![allow(unused)]
        // Example stub:
        let name_to_use = account_name.to_string();
        let user = self.user_repo.get_by_global_username(&name_to_use).await?
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
        // (unchanged from earlier) ...
        #![allow(unused)]
        Ok(())
    }

    // (NEW) Expose an “instance” function.
    async fn vrchat_get_current_instance(&self, account_name: &str) -> Result<VRChatInstanceBasic, Error> {
        // 1) If user didn't provide account_name, pick the single VRChat account. Same logic as above.
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

        // 2) Look up the user row
        let user = self.user_repo
            .get_by_global_username(&name_to_use)
            .await?
            .ok_or_else(|| {
                Error::Platform(format!("No user found with global_username='{name_to_use}'"))
            })?;

        // 3) Grab that user’s VRChat credential
        let cred_opt = {
            if let Some(am) = &self.auth_manager {
                let lock = am.lock().await;
                lock.credentials_repo.get_credentials(&Platform::VRChat, user.user_id).await?
            } else {
                return Err(Error::Auth("No auth manager set in plugin manager".into()));
            }
        };
        let cred = match cred_opt {
            Some(c) => c,
            None => return Err(Error::Platform(
                format!("No VRChat credential for user='{}'", name_to_use)
            )),
        };

        // 4) Create the VRChatClient; call fetch_current_instance_api()
        let client = VRChatClient::new(&cred.primary_token)?;
        let inst_opt = client.fetch_current_instance_api().await?;

        // 5) If offline or no instance, return an error or indicate offline
        let inst = match inst_opt {
            Some(i) => i,
            None => return Err(Error::Platform("User is offline or has no instance.".into())),
        };

        // 6) Convert that to our simpler VRChatInstanceBasic
        Ok(VRChatInstanceBasic {
            world_id: inst.world_id,
            instance_id: inst.instance_id,
            location: inst.location,
        })
    }
}