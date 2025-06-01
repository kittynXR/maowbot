// maowbot-core/src/plugins/manager/drip_api_impl.rs
//
// Implementation of the DripApi trait for PluginManager.
// This is just a demonstration skeleton; real logic is to be expanded.

use async_trait::async_trait;
use maowbot_common::models::drip::{DripAvatarSummary};
use maowbot_common::traits::api::DripApi;

use crate::Error;
use crate::plugins::manager::core::PluginManager;

#[async_trait]
impl DripApi for PluginManager {
    async fn drip_show_settable(&self) -> Result<String, Error> {
        // For demonstration, just listing which two prefix-rules we support:
        Ok("Settable prefix rules: i/ignore <prefix>, s/strip <prefix>, name <newName>".to_string())
    }

    async fn drip_set_ignore_prefix(&self, prefix: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        let current_avatar = repo.current_avatar()?.ok_or_else(|| {
            Error::Platform("No current avatar is selected.".to_string())
        })?;
        repo.add_prefix_rule_ignore(&current_avatar.drip_avatar_id, prefix).await?;
        Ok(format!("Ignoring all params with prefix '{}'", prefix))
    }

    async fn drip_set_strip_prefix(&self, prefix: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        let current_avatar = repo.current_avatar()?.ok_or_else(|| {
            Error::Platform("No current avatar is selected.".to_string())
        })?;
        repo.add_prefix_rule_strip(&current_avatar.drip_avatar_id, prefix).await?;
        Ok(format!("Stripping prefix '{}' from params", prefix))
    }

    async fn drip_set_avatar_name(&self, new_name: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        let curr = repo.current_avatar()?.ok_or_else(|| {
            Error::Platform("No current avatar is selected.".to_string())
        })?;
        repo.update_local_avatar_name(&curr.drip_avatar_id, new_name).await?;
        Ok(format!("Local avatar renamed to '{}'", new_name))
    }

    async fn drip_list_avatars(&self) -> Result<Vec<DripAvatarSummary>, Error> {
        let repo = self.drip_repo.clone();
        let list = repo.list_avatars().await?;
        let out = list.into_iter().map(|av| DripAvatarSummary {
            vrchat_avatar_id: av.vrchat_avatar_id,
            vrchat_avatar_name: av.vrchat_avatar_name,
            local_name: av.local_name,
        }).collect();
        Ok(out)
    }

    async fn drip_fit_new(&self, fit_name: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        let curr = repo.current_avatar()?.ok_or_else(|| {
            Error::Platform("No current avatar selected.".to_string())
        })?;
        repo.create_fit(&curr.drip_avatar_id, fit_name).await?;
        Ok(format!("Created new outfit '{}'", fit_name))
    }

    async fn drip_fit_add_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        repo.add_fit_param(fit_name, param_name, param_value).await?;
        Ok(format!("Added param {}={} to fit '{}'", param_name, param_value, fit_name))
    }

    async fn drip_fit_del_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        repo.del_fit_param(fit_name, param_name, param_value).await?;
        Ok(format!("Removed param {}={} from fit '{}'", param_name, param_value, fit_name))
    }

    async fn drip_fit_wear(&self, fit_name: &str) -> Result<String, Error> {
        // 1) Retrieve all param pairs from DB
        // 2) For each, try sending an OSC param
        // 3) If param is not in the VRChat config, print a warning
        let repo = self.drip_repo.clone();
        let fit_params = repo.get_fit_params(fit_name).await?;

        // (In real usage, you'd also ensure the avatar is actually loaded, etc.)
        let mut missing_params = Vec::new();
        for fp in &fit_params {
            // Check if param is known in the discovered list:
            let known = repo.is_param_known_for_current_avatar(fp.param_name.clone()).await?;
            if !known {
                missing_params.push(fp.param_name.clone());
            }
            // In production, also send an actual OSC message: e.g. "self.osc_manager..."
        }

        if !missing_params.is_empty() {
            return Ok(format!(
                "Fit '{}' is worn, but these param(s) are missing on the avatar: {:?}",
                fit_name, missing_params
            ));
        }
        Ok(format!("Fit '{}' was successfully worn (all params exist).", fit_name))
    }

    async fn drip_props_add(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        repo.add_prop_param(prop_name, param_name, param_value).await?;
        Ok(format!("Added prop param: {} = {}", param_name, param_value))
    }

    async fn drip_props_del(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        repo.del_prop_param(prop_name, param_name, param_value).await?;
        Ok(format!("Removed prop param: {} = {}", param_name, param_value))
    }

    async fn drip_props_timer(&self, prop_name: &str, timer_data: &str) -> Result<String, Error> {
        let repo = self.drip_repo.clone();
        repo.add_prop_timer(prop_name, timer_data).await?;
        Ok(format!("Timer data set for prop '{}': {}", prop_name, timer_data))
    }
}
