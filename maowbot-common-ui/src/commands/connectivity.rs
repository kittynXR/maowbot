use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    StartPlatformRuntimeRequest, StopPlatformRuntimeRequest, RuntimeConfig,
    JoinChannelRequest, ListCredentialsRequest, GetUserRequest,
    ListAutostartEntriesRequest, SetAutostartRequest,
};
use std::collections::HashMap;

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
    /// Configure autostart for a platform account using the gRPC service
    pub async fn configure_autostart(
        client: &GrpcClient,
        enable: bool,
        platform: &str,
        account: &str,
    ) -> Result<AutostartResult, CommandError> {
        let request = SetAutostartRequest {
            platform: platform.to_string(),
            account_name: account.to_string(),
            enabled: enable,
        };
        
        let mut autostart_client = client.autostart.clone();
        let response = autostart_client
            .set_autostart(request)
            .await
            .map_err(|e| CommandError::GrpcError(format!("Failed to set autostart: {}", e)))?;
            
        let response = response.into_inner();
        
        if !response.success {
            return Err(CommandError::GrpcError(response.message));
        }
        
        Ok(AutostartResult {
            platform: platform.to_string(),
            account: account.to_string(),
            enabled: enable,
        })
    }
    
    /// List autostart entries
    pub async fn list_autostart_entries(
        client: &GrpcClient,
    ) -> Result<Vec<(String, String, bool)>, CommandError> {
        let request = ListAutostartEntriesRequest {
            enabled_only: false,
        };
        
        let mut autostart_client = client.autostart.clone();
        let response = autostart_client
            .list_autostart_entries(request)
            .await
            .map_err(|e| CommandError::GrpcError(format!("Failed to list autostart entries: {}", e)))?;
            
        let response = response.into_inner();
        
        Ok(response.entries.into_iter()
            .map(|e| (e.platform, e.account_name, e.enabled))
            .collect())
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
        // Parse platform string to enum value
        let platform_enum = match platform.to_lowercase().as_str() {
            "twitch" | "twitch-helix" | "twitchhelix" => maowbot_proto::maowbot::common::Platform::TwitchHelix,
            "twitch-irc" | "twitchirc" => maowbot_proto::maowbot::common::Platform::TwitchIrc,
            "twitch-eventsub" | "twitcheventsub" => maowbot_proto::maowbot::common::Platform::TwitchEventsub,
            "discord" => maowbot_proto::maowbot::common::Platform::Discord,
            "vrchat" => maowbot_proto::maowbot::common::Platform::Vrchat,
            "vrchat-pipeline" | "vrchatpipeline" => maowbot_proto::maowbot::common::Platform::VrchatPipeline,
            _ => return Err(CommandError::InvalidInput(format!("Unknown platform: {}", platform))),
        };
            
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
        
        // Get user details for each credential and deduplicate by user_name
        let mut accounts = Vec::new();
        let mut seen_usernames = std::collections::HashSet::new();
        let mut user_client = client.user.clone();
        
        for cred_info in credentials {
            let credential = cred_info.credential.ok_or_else(|| 
                CommandError::DataError("Missing credential data".to_string()))?;
            
            // Skip if we've already seen this username
            if !seen_usernames.insert(credential.user_name.clone()) {
                continue;
            }
                
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