use tokio::sync::mpsc;

use crate::Error;
use crate::eventbus::{BotEvent};
use maowbot_common::models::analytics as common_analytics;
use maowbot_common::traits::api::{PluginApi, CredentialsApi};
use maowbot_common::models::plugin::{StatusData, AccountStatus};
use crate::plugins::manager::core::PluginManager;
use crate::repositories::postgres::user::UserRepo;

/// Helper function to build a `StatusData`.
pub async fn build_status_response(manager: &PluginManager) -> maowbot_proto::plugs::PluginStreamResponse {
    use maowbot_proto::plugs::plugin_stream_response::Payload as RespPayload;
    use maowbot_proto::plugs::StatusResponse;

    let connected = {
        let infos = manager.list_connected_plugins().await;
        infos.into_iter().map(|i| i.name).collect::<Vec<_>>()
    };
    let uptime = manager.start_time.elapsed().as_secs();
    let response = maowbot_proto::plugs::PluginStreamResponse {
        payload: Some(RespPayload::StatusResponse(StatusResponse {
            connected_plugins: connected,
            server_uptime: uptime,
        })),
    };
    response
}

/// Convert our local `eventbus::BotEvent` to the new `maowbot_common::models::analytics::BotEvent`.
fn convert_event(evt: crate::eventbus::BotEvent) -> common_analytics::BotEvent {
    match evt {
        BotEvent::ChatMessage {
            platform,
            channel,
            user,
            text,
            timestamp,
        } => {
            common_analytics::BotEvent {
                event_id: uuid::Uuid::new_v4(),
                event_type: "chat_message".to_string(),
                event_timestamp: timestamp,
                data: Some(serde_json::json!({
                    "platform": platform,
                    "channel": channel,
                    "user": user,
                    "text": text,
                })),
            }
        }
        BotEvent::Tick => {
            common_analytics::BotEvent {
                event_id: uuid::Uuid::new_v4(),
                event_type: "tick".to_string(),
                event_timestamp: chrono::Utc::now(),
                data: None,
            }
        }
        BotEvent::SystemMessage(msg) => {
            common_analytics::BotEvent {
                event_id: uuid::Uuid::new_v4(),
                event_type: "system_message".to_string(),
                event_timestamp: chrono::Utc::now(),
                data: Some(serde_json::json!({ "message": msg })),
            }
        }
        BotEvent::TwitchEventSub(sub) => {
            // If desired, store more structured data from `sub`:
            common_analytics::BotEvent {
                event_id: uuid::Uuid::new_v4(),
                event_type: "twitch_eventsub".to_string(),
                event_timestamp: chrono::Utc::now(),
                data: Some(serde_json::json!({
                    "details": format!("{:?}", sub)
                })),
            }
        }
    }
}

#[async_trait::async_trait]
impl PluginApi for PluginManager {
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
        // Gather connected plugin names:
        let connected = self.list_connected_plugins().await;
        let connected_names: Vec<_> = connected
            .into_iter()
            .map(|p| {
                let suffix = if p.is_enabled { "" } else { " (disabled)" };
                format!("{}{}", p.name, suffix)
            })
            .collect();

        // Gather every stored credential (so we can see which are connected)
        let creds_result = self.list_credentials(None).await; // from CredentialsApi
        let mut account_statuses = Vec::new();

        if let Ok(all_creds) = creds_result {
            let guard = self.platform_manager.active_runtimes.lock().await;
            for c in all_creds {
                // Attempt to read the user’s display name
                let user_display = match self.user_repo.get(c.user_id).await {
                    Ok(Some(u)) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
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
        let maybe_rec = {
            let lock = self.plugin_records.lock().unwrap();
            lock.iter().find(|r| r.name == plugin_name).cloned()
        };
        let rec = match maybe_rec {
            Some(r) => r,
            None => return Err(Error::Platform(format!("No known plugin named '{}'", plugin_name))),
        };

        if rec.enabled == enable {
            return Ok(());
        }
        let updated = crate::plugins::types::PluginRecord {
            name: rec.name.clone(),
            plugin_type: rec.plugin_type.clone(),
            enabled: enable,
        };
        self.upsert_plugin_record(updated.clone());
        let action_str = if enable { "ENABLED" } else { "DISABLED" };
        tracing::info!("PluginManager: set plugin '{}' to {}", updated.name, action_str);

        match updated.plugin_type {
            crate::plugins::types::PluginType::Grpc => {
                // Look for the connection in memory and call set_enabled
                let lock = self.plugins.lock().await;
                for p in lock.iter() {
                    let pi = p.info().await;
                    if pi.name == updated.name {
                        p.set_capabilities(pi.capabilities.clone()).await; // re-send caps
                        p.set_enabled(enable).await;
                        break;
                    }
                }
            }
            crate::plugins::types::PluginType::DynamicLib { .. } => {
                if enable {
                    // If not loaded yet, actually load it:
                    let lock = self.plugins.lock().await;
                    let already_loaded = lock.iter().any(|p| {
                        let pi = futures_lite::future::block_on(p.info());
                        pi.name == updated.name
                    });
                    drop(lock);

                    if !already_loaded {
                        if let Err(e) = self.load_in_process_plugin_by_record(&updated).await {
                            tracing::error!("Failed to load '{}': {:?}", updated.name, e);
                        }
                    } else {
                        // If it’s already in memory, just enable it:
                        let lock = self.plugins.lock().await;
                        for p in lock.iter() {
                            let pi = p.info().await;
                            if pi.name == updated.name {
                                p.set_capabilities(pi.capabilities.clone()).await;
                                p.set_enabled(true).await;
                                break;
                            }
                        }
                    }
                } else {
                    // If disabling => remove from memory:
                    let mut lock = self.plugins.lock().await;
                    if let Some(i) = lock.iter().position(|p| {
                        let pi = futures_lite::future::block_on(p.info());
                        pi.name == updated.name
                    }) {
                        let plugin_arc = lock.remove(i);
                        let _ = plugin_arc.stop().await;
                        tracing::info!("Unloaded in-process plugin '{}'", updated.name);
                    }
                }
            }
        }
        Ok(())
    }

    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error> {
        let maybe_record = {
            let lock = self.plugin_records.lock().unwrap();
            lock.iter().find(|r| r.name == plugin_name).cloned()
        };
        let record = match maybe_record {
            Some(r) => r,
            None => {
                return Err(Error::Platform(format!("No known plugin named '{}'", plugin_name)));
            }
        };

        // If plugin is loaded in memory, remove it
        {
            let mut lock = self.plugins.lock().await;
            if let Some(pos) = lock.iter().position(|p| {
                let pi = futures_lite::future::block_on(p.info());
                pi.name == record.name
            }) {
                let plugin_arc = lock.remove(pos);
                let _ = plugin_arc.stop().await;
                tracing::info!("Stopped and removed in-memory plugin '{}'", record.name);
            }
        }

        // Remove from plugin_records
        {
            let mut lock = self.plugin_records.lock().unwrap();
            lock.retain(|r| r.name != record.name);
        }
        self.save_plugin_states();
        tracing::info!("Plugin '{}' removed from JSON records.", plugin_name);

        Ok(())
    }

    // ------------------------------------------------------------
    // The method that caused the E0053 type mismatch. We now convert
    // from `crate::eventbus::BotEvent` to `maowbot_common::models::analytics::BotEvent`.
    // ------------------------------------------------------------
    async fn subscribe_chat_events(
        &self,
        buffer_size: Option<usize>
    ) -> mpsc::Receiver<common_analytics::BotEvent> {
        if let Some(bus) = &self.event_bus {
            // We subscribe to the local eventbus (which yields `crate::eventbus::BotEvent`).
            let raw_rx = bus.subscribe(buffer_size).await;
            let (tx, rx) = mpsc::channel(buffer_size.unwrap_or(128));

            // Spawn a small forwarder task to convert & forward each event.
            tokio::spawn(async move {
                let mut raw_rx = raw_rx;
                while let Some(evt) = raw_rx.recv().await {
                    let converted = convert_event(evt);
                    if tx.send(converted).await.is_err() {
                        break;
                    }
                }
            });

            rx
        } else {
            // If there's no event bus at all, return a dummy channel that never yields data.
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
}
