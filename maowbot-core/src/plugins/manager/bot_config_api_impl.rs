// File: maowbot-core/src/plugins/manager/bot_config_api_impl.rs

use async_trait::async_trait;
use serde_json::Value;
use crate::Error;
use crate::plugins::manager::core::PluginManager;
use maowbot_common::traits::api::BotConfigApi;

#[async_trait]
impl BotConfigApi for PluginManager {
    async fn list_all_config(&self) -> Result<Vec<(String, String)>, Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager set in plugin manager".into()))?;
        let auth_manager_locked = auth_mgr_arc.lock().await;
        auth_manager_locked.bot_config_repo.list_all().await
    }

    async fn get_bot_config_value(&self, config_key: &str) -> Result<Option<String>, Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager".into()))?;
        let lock = auth_mgr_arc.lock().await;
        lock.bot_config_repo.get_value(config_key).await
    }

    async fn set_bot_config_value(&self, config_key: &str, config_value: &str) -> Result<(), Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager".into()))?;
        let lock = auth_mgr_arc.lock().await;
        lock.bot_config_repo.set_value(config_key, config_value).await
    }

    async fn delete_bot_config_key(&self, config_key: &str) -> Result<(), Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager".into()))?;
        let lock = auth_mgr_arc.lock().await;
        lock.bot_config_repo.delete_value(config_key).await
    }

    async fn set_config_kv_meta(
        &self,
        config_key: &str,
        config_value: &str,
        config_meta: Option<Value>
    ) -> Result<(), Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager".into()))?;
        let lock = auth_mgr_arc.lock().await;
        lock.bot_config_repo.set_value_kv_meta(config_key, config_value, config_meta).await
    }

    async fn get_config_kv_meta(
        &self,
        config_key: &str,
        config_value: &str
    ) -> Result<Option<(String, Option<Value>)>, Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager".into()))?;
        let lock = auth_mgr_arc.lock().await;
        lock.bot_config_repo.get_value_kv_meta(config_key, config_value).await
    }

    async fn delete_config_kv(&self, config_key: &str, config_value: &str) -> Result<(), Error> {
        let auth_mgr_arc = self.auth_manager
            .as_ref()
            .ok_or_else(|| Error::Auth("No auth manager".into()))?;
        let lock = auth_mgr_arc.lock().await;
        lock.bot_config_repo.delete_value_kv(config_key, config_value).await
    }
}
