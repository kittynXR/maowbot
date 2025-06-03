use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    GetConfigRequest, SetConfigRequest, ListConfigsRequest,
    ListAvatarsRequest, AvatarListType,
};
use serde::{Serialize, Deserialize};

/// Result of setting drip configuration
pub struct DripSetResult {
    pub setting_type: String,
    pub value: String,
}

/// Avatar information for drip
pub struct DripAvatar {
    pub vrchat_avatar_id: String,
    pub vrchat_avatar_name: String,
    pub local_name: Option<String>,
}

/// Fit information
pub struct DripFit {
    pub name: String,
    pub parameters: Vec<(String, String)>,
}

/// Drip command handlers
pub struct DripCommands;

impl DripCommands {
    /// Get drip configuration value
    async fn get_drip_config(client: &GrpcClient, key: &str) -> Result<String, CommandError> {
        let request = GetConfigRequest {
            key: format!("drip.{}", key),
            include_metadata: false,
        };
        
        let mut client = client.config.clone();
        match client.get_config(request).await {
            Ok(response) => {
                Ok(response.into_inner()
                    .config
                    .map(|c| c.value)
                    .unwrap_or_default())
            }
            Err(_) => Ok(String::new()),
        }
    }
    
    /// Set drip configuration value
    async fn set_drip_config(client: &GrpcClient, key: &str, value: &str) -> Result<(), CommandError> {
        let request = SetConfigRequest {
            key: format!("drip.{}", key),
            value: value.to_string(),
            metadata: None,
            validate_only: false,
        };
        
        let mut client = client.config.clone();
        client
            .set_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Show settable drip options
    pub async fn show_settable(client: &GrpcClient) -> Result<Vec<(String, String)>, CommandError> {
        let request = ListConfigsRequest {
            categories: vec![],
            include_secrets: false,
            include_metadata: false,
            key_prefix: "drip.".to_string(),
            page: None,
        };
        
        let mut client = client.config.clone();
        let response = client
            .list_configs(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let configs = response.into_inner().configs;
        
        let mut settings = vec![];
        for config in configs {
            if let Some(key) = config.key.strip_prefix("drip.") {
                settings.push((key.to_string(), config.value));
            }
        }
        
        // Add default settings if not present
        if !settings.iter().any(|(k, _)| k == "ignore_prefix") {
            settings.push(("ignore_prefix".to_string(), "(not set)".to_string()));
        }
        if !settings.iter().any(|(k, _)| k == "strip_prefix") {
            settings.push(("strip_prefix".to_string(), "(not set)".to_string()));
        }
        if !settings.iter().any(|(k, _)| k == "avatar_name") {
            settings.push(("avatar_name".to_string(), "(not set)".to_string()));
        }
        
        Ok(settings)
    }
    
    /// Set ignore prefix
    pub async fn set_ignore_prefix(client: &GrpcClient, prefix: &str) -> Result<DripSetResult, CommandError> {
        Self::set_drip_config(client, "ignore_prefix", prefix).await?;
        Ok(DripSetResult {
            setting_type: "ignore_prefix".to_string(),
            value: prefix.to_string(),
        })
    }
    
    /// Set strip prefix
    pub async fn set_strip_prefix(client: &GrpcClient, prefix: &str) -> Result<DripSetResult, CommandError> {
        Self::set_drip_config(client, "strip_prefix", prefix).await?;
        Ok(DripSetResult {
            setting_type: "strip_prefix".to_string(),
            value: prefix.to_string(),
        })
    }
    
    /// Set avatar name
    pub async fn set_avatar_name(client: &GrpcClient, name: &str) -> Result<DripSetResult, CommandError> {
        Self::set_drip_config(client, "avatar_name", name).await?;
        Ok(DripSetResult {
            setting_type: "avatar_name".to_string(),
            value: name.to_string(),
        })
    }
    
    /// List avatars from VRChat
    pub async fn list_avatars(client: &GrpcClient, account_name: &str) -> Result<Vec<DripAvatar>, CommandError> {
        let request = ListAvatarsRequest {
            account_name: account_name.to_string(),
            list_type: AvatarListType::Mine as i32,
            page: None,
        };
        
        let mut vrchat_client = client.vrchat.clone();
        let response = vrchat_client
            .list_avatars(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let avatars = response.into_inner().avatars;
        
        // Get local names from config
        let mut drip_avatars = vec![];
        for avatar in avatars {
            let local_name = Self::get_drip_config(client, &format!("avatar.{}.name", avatar.avatar_id))
                .await
                .ok()
                .filter(|s| !s.is_empty());
                
            drip_avatars.push(DripAvatar {
                vrchat_avatar_id: avatar.avatar_id,
                vrchat_avatar_name: avatar.name,
                local_name,
            });
        }
        
        Ok(drip_avatars)
    }
    
    /// Create a new fit
    pub async fn fit_new(client: &GrpcClient, fit_name: &str) -> Result<(), CommandError> {
        // Store empty fit configuration
        let fit_config = DripFitConfig {
            name: fit_name.to_string(),
            parameters: vec![],
        };
        
        let json = serde_json::to_string(&fit_config)
            .map_err(|e| CommandError::DataError(format!("Failed to serialize fit: {}", e)))?;
            
        Self::set_drip_config(client, &format!("fit.{}", fit_name), &json).await?;
        Ok(())
    }
    
    /// Add parameter to fit
    pub async fn fit_add_param(
        client: &GrpcClient,
        fit_name: &str,
        param: &str,
        value: &str,
    ) -> Result<(), CommandError> {
        let mut fit_config = Self::get_fit_config(client, fit_name).await?;
        
        // Add or update parameter
        if let Some(existing) = fit_config.parameters.iter_mut().find(|(p, _)| p == param) {
            existing.1 = value.to_string();
        } else {
            fit_config.parameters.push((param.to_string(), value.to_string()));
        }
        
        let json = serde_json::to_string(&fit_config)
            .map_err(|e| CommandError::DataError(format!("Failed to serialize fit: {}", e)))?;
            
        Self::set_drip_config(client, &format!("fit.{}", fit_name), &json).await?;
        Ok(())
    }
    
    /// Remove parameter from fit
    pub async fn fit_del_param(
        client: &GrpcClient,
        fit_name: &str,
        param: &str,
        value: &str,
    ) -> Result<(), CommandError> {
        let mut fit_config = Self::get_fit_config(client, fit_name).await?;
        
        // Remove matching parameter
        fit_config.parameters.retain(|(p, v)| !(p == param && v == value));
        
        let json = serde_json::to_string(&fit_config)
            .map_err(|e| CommandError::DataError(format!("Failed to serialize fit: {}", e)))?;
            
        Self::set_drip_config(client, &format!("fit.{}", fit_name), &json).await?;
        Ok(())
    }
    
    /// Get fit configuration
    async fn get_fit_config(client: &GrpcClient, fit_name: &str) -> Result<DripFitConfig, CommandError> {
        let json = Self::get_drip_config(client, &format!("fit.{}", fit_name)).await?;
        
        if json.is_empty() {
            return Err(CommandError::NotFound(format!("Fit '{}' not found", fit_name)));
        }
        
        serde_json::from_str(&json)
            .map_err(|e| CommandError::DataError(format!("Failed to parse fit config: {}", e)))
    }
    
    /// Wear a fit (apply parameters via OSC)
    pub async fn fit_wear(client: &GrpcClient, fit_name: &str) -> Result<DripFit, CommandError> {
        let fit_config = Self::get_fit_config(client, fit_name).await?;
        
        // Note: Actually sending OSC parameters would require the OSC service
        // For now, we just return the fit information
        Ok(DripFit {
            name: fit_config.name,
            parameters: fit_config.parameters,
        })
    }
    
    /// Add prop configuration
    pub async fn props_add(
        client: &GrpcClient,
        prop_name: &str,
        param: &str,
        value: &str,
    ) -> Result<(), CommandError> {
        let mut prop_config = Self::get_prop_config(client, prop_name).await.unwrap_or_default();
        
        // Add or update parameter
        prop_config.set_parameter(param, value);
        
        let json = serde_json::to_string(&prop_config)
            .map_err(|e| CommandError::DataError(format!("Failed to serialize prop: {}", e)))?;
            
        Self::set_drip_config(client, &format!("props.{}", prop_name), &json).await?;
        Ok(())
    }
    
    /// Remove prop configuration
    pub async fn props_del(
        client: &GrpcClient,
        prop_name: &str,
        param: &str,
        value: &str,
    ) -> Result<(), CommandError> {
        let mut prop_config = Self::get_prop_config(client, prop_name).await?;
        
        // Remove matching parameter
        prop_config.remove_parameter(param, value);
        
        let json = serde_json::to_string(&prop_config)
            .map_err(|e| CommandError::DataError(format!("Failed to serialize prop: {}", e)))?;
            
        Self::set_drip_config(client, &format!("props.{}", prop_name), &json).await?;
        Ok(())
    }
    
    /// Set prop timer
    pub async fn props_timer(
        client: &GrpcClient,
        prop_name: &str,
        timer_data: &str,
    ) -> Result<(), CommandError> {
        let mut prop_config = Self::get_prop_config(client, prop_name).await.unwrap_or_default();
        
        prop_config.timer = Some(timer_data.to_string());
        
        let json = serde_json::to_string(&prop_config)
            .map_err(|e| CommandError::DataError(format!("Failed to serialize prop: {}", e)))?;
            
        Self::set_drip_config(client, &format!("props.{}", prop_name), &json).await?;
        Ok(())
    }
    
    /// Get prop configuration
    async fn get_prop_config(client: &GrpcClient, prop_name: &str) -> Result<DripPropConfig, CommandError> {
        let json = Self::get_drip_config(client, &format!("props.{}", prop_name)).await?;
        
        if json.is_empty() {
            return Ok(DripPropConfig::default());
        }
        
        serde_json::from_str(&json)
            .map_err(|e| CommandError::DataError(format!("Failed to parse prop config: {}", e)))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DripFitConfig {
    name: String,
    parameters: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct DripPropConfig {
    parameters: Vec<(String, String)>,
    timer: Option<String>,
}

impl DripPropConfig {
    fn set_parameter(&mut self, param: &str, value: &str) {
        if let Some(existing) = self.parameters.iter_mut().find(|(p, _)| p == param) {
            existing.1 = value.to_string();
        } else {
            self.parameters.push((param.to_string(), value.to_string()));
        }
    }
    
    fn remove_parameter(&mut self, param: &str, value: &str) {
        self.parameters.retain(|(p, v)| !(p == param && v == value));
    }
}