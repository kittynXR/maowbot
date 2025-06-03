use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    StartPlatformRuntimeRequest, StopPlatformRuntimeRequest, RuntimeConfig,
    JoinChannelRequest, GetConfigRequest, SetConfigRequest,
    ListCredentialsRequest, GetUserRequest,
};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Result of autostart configuration
pub struct AutostartResult {
    pub platform: String,
    pub account: String,
    pub enabled: bool,
}

/// Result of starting a platform
pub struct StartPlatformResult {
    pub platform: String,
    pub account: String,
    pub runtime_id: Option<String>,
}

/// Result of stopping a platform
pub struct StopPlatformResult {
    pub platform: String,
    pub account: String,
}

/// Available platform info for selection
pub struct PlatformAccountInfo {
    pub user_id: String,
    pub user_name: String,
    pub display_name: String,
}

/// Connectivity command handlers
pub struct ConnectivityCommands;

impl ConnectivityCommands {
    /// Configure autostart for a platform account
    pub async fn configure_autostart(
        client: &GrpcClient,
        enable: bool,
        platform: &str,
        account: &str,
    ) -> Result<AutostartResult, CommandError> {
        // Get current autostart config
        let get_request = GetConfigRequest {
            key: "autostart".to_string(),
            include_metadata: false,
        };
        
        let mut config_client = client.config.clone();
        let current_val = match config_client.get_config(get_request).await {
            Ok(response) => {
                response.into_inner()
                    .config
                    .map(|c| c.value)
                    .unwrap_or_default()
            }
            Err(_) => String::new(),
        };
        
        // Parse or create autostart config
        let mut config_obj: AutostartConfig = if current_val.is_empty() {
            AutostartConfig::new()
        } else {
            serde_json::from_str(&current_val)
                .unwrap_or_else(|_| AutostartConfig::new())
        };
        
        // Update the config
        config_obj.set_platform_account(platform, account, enable);
        
        // Save back to bot_config
        let new_str = serde_json::to_string_pretty(&config_obj)
            .map_err(|e| CommandError::DataError(format!("Failed to serialize autostart config: {}", e)))?;
            
        let set_request = SetConfigRequest {
            key: "autostart".to_string(),
            value: new_str,
            metadata: None,
            validate_only: false,
        };
        
        config_client
            .set_config(set_request)
            .await
            .map_err(|e| CommandError::GrpcError(format!("Failed to save autostart config: {}", e)))?;
            
        Ok(AutostartResult {
            platform: platform.to_string(),
            account: account.to_string(),
            enabled: enable,
        })
    }
    
    /// Start a platform runtime
    pub async fn start_platform(
        client: &GrpcClient,
        platform: &str,
        account: &str,
    ) -> Result<StartPlatformResult, CommandError> {
        let request = StartPlatformRuntimeRequest {
            platform: platform.to_string(),
            account_name: account.to_string(),
            config: Some(RuntimeConfig {
                auto_reconnect: true,
                reconnect_delay_seconds: 5,
                platform_specific: HashMap::new(),
            }),
        };
        
        let mut platform_client = client.platform.clone();
        let response = platform_client
            .start_platform_runtime(request)
            .await
            .map_err(|e| CommandError::GrpcError(format!("Failed to start platform: {}", e)))?;
            
        let response = response.into_inner();
        
        if !response.error_message.is_empty() {
            return Err(CommandError::GrpcError(response.error_message));
        }
        
        Ok(StartPlatformResult {
            platform: platform.to_string(),
            account: account.to_string(),
            runtime_id: Some(response.runtime_id),
        })
    }
    
    /// Stop a platform runtime
    pub async fn stop_platform(
        client: &GrpcClient,
        platform: &str,
        account: &str,
    ) -> Result<StopPlatformResult, CommandError> {
        let request = StopPlatformRuntimeRequest {
            platform: platform.to_string(),
            account_name: account.to_string(),
            force: false,
        };
        
        let mut platform_client = client.platform.clone();
        platform_client
            .stop_platform_runtime(request)
            .await
            .map_err(|e| CommandError::GrpcError(format!("Failed to stop platform: {}", e)))?;
            
        Ok(StopPlatformResult {
            platform: platform.to_string(),
            account: account.to_string(),
        })
    }
    
    /// List all accounts for a platform
    pub async fn list_platform_accounts(
        client: &GrpcClient,
        platform: &str,
    ) -> Result<Vec<PlatformAccountInfo>, CommandError> {
        let platform_enum = maowbot_proto::maowbot::common::Platform::from_str_name(platform)
            .ok_or_else(|| CommandError::InvalidInput(format!("Unknown platform: {}", platform)))?;
            
        let request = ListCredentialsRequest {
            platforms: vec![platform_enum as i32],
            include_expired: false,
            active_only: true,
            page: None,
        };
        
        let mut cred_client = client.credential.clone();
        let response = cred_client
            .list_credentials(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let credentials = response.into_inner().credentials;
        
        // Get user details for each credential
        let mut accounts = Vec::new();
        let mut user_client = client.user.clone();
        
        for cred_info in credentials {
            let credential = cred_info.credential.ok_or_else(|| 
                CommandError::DataError("Missing credential data".to_string()))?;
                
            let user_request = GetUserRequest {
                user_id: credential.user_id.clone(),
                include_identities: false,
                include_analysis: false,
            };
            
            let display_name = match user_client.get_user(user_request).await {
                Ok(resp) => {
                    resp.into_inner()
                        .user
                        .and_then(|u| Some(u.global_username))
                        .unwrap_or_else(|| credential.user_id.clone())
                }
                Err(_) => credential.user_id.clone(),
            };
            
            accounts.push(PlatformAccountInfo {
                user_id: credential.user_id,
                user_name: credential.user_name,
                display_name,
            });
        }
        
        Ok(accounts)
    }
    
    /// Join a Twitch IRC channel
    pub async fn join_twitch_channel(
        client: &GrpcClient,
        account: &str,
        channel: &str,
    ) -> Result<(), CommandError> {
        let request = JoinChannelRequest {
            account_name: account.to_string(),
            channel: channel.to_string(),
        };
        
        let mut twitch_client = client.twitch.clone();
        twitch_client
            .join_channel(request)
            .await
            .map_err(|e| CommandError::GrpcError(format!("Failed to join channel: {}", e)))?;
            
        Ok(())
    }
}

// AutostartConfig structure matching the core implementation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AutostartConfig {
    pub twitch_irc: Vec<String>,
    pub twitch_eventsub: Vec<String>,
    pub discord: Vec<String>,
    pub vrchat: Vec<String>,
}

impl AutostartConfig {
    pub fn new() -> Self {
        Self {
            twitch_irc: Vec::new(),
            twitch_eventsub: Vec::new(),
            discord: Vec::new(),
            vrchat: Vec::new(),
        }
    }
    
    pub fn set_platform_account(&mut self, platform: &str, account: &str, enable: bool) {
        let list = match platform.to_lowercase().as_str() {
            "twitch-irc" => &mut self.twitch_irc,
            "twitch-eventsub" => &mut self.twitch_eventsub,
            "discord" => &mut self.discord,
            "vrchat" => &mut self.vrchat,
            _ => return,
        };
        
        if enable {
            if !list.iter().any(|a| a.eq_ignore_ascii_case(account)) {
                list.push(account.to_string());
            }
        } else {
            list.retain(|a| !a.eq_ignore_ascii_case(account));
        }
    }
}