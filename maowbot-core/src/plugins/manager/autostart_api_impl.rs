use async_trait::async_trait;
use maowbot_common::traits::api::AutostartApi;
use maowbot_common::error::Error;
use crate::plugins::manager::PluginManager;

#[async_trait]
impl AutostartApi for PluginManager {
    async fn list_autostart_entries(&self) -> Result<Vec<(String, String, bool)>, Error> {
        let entries = self.autostart_repo.get_all_entries().await?;
        Ok(entries.into_iter()
            .map(|e| (e.platform, e.account_name, e.enabled))
            .collect())
    }
    
    async fn set_autostart(&self, platform: &str, account: &str, enabled: bool) -> Result<(), Error> {
        self.autostart_repo.set_autostart(platform, account, enabled).await
    }
    
    async fn is_autostart_enabled(&self, platform: &str, account: &str) -> Result<bool, Error> {
        self.autostart_repo.is_autostart_enabled(platform, account).await
    }
}