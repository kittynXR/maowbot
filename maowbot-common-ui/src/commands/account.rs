use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    BeginAuthFlowRequest, ListCredentialsRequest, RefreshCredentialRequest, 
    RevokeCredentialRequest, StoreCredentialRequest, PlatformUserIdentifier,
};
use maowbot_proto::maowbot::common::Platform;

/// Convert Platform enum to user-friendly string
fn platform_to_display_string(platform: Platform) -> &'static str {
    match platform {
        Platform::TwitchHelix => "TwitchHelix",
        Platform::TwitchIrc => "TwitchIrc",
        Platform::TwitchEventsub => "TwitchEventSub",
        Platform::Discord => "Discord",
        Platform::Vrchat => "VRChat",
        Platform::VrchatPipeline => "VRChatPipeline",
        Platform::Obs => "OBS",
        Platform::Unknown => "Unknown",
    }
}

/// Result of adding an account
pub struct AddAccountResult {
    pub message: String,
    pub auth_url: Option<String>,
    pub state: Option<String>,
}

/// Result of removing an account  
pub struct RemoveAccountResult {
    pub message: String,
}

/// Result of listing accounts
pub struct ListAccountsResult {
    pub credentials: Vec<CredentialInfo>,
}

pub struct CredentialInfo {
    pub username: String,
    pub platform: String,
    pub is_bot: bool,
    pub credential_id: String,
    pub user_id: String,
}

/// Result of showing account details
pub struct ShowAccountResult {
    pub credential: Option<CredentialDetail>,
}

pub struct CredentialDetail {
    pub platform: String,
    pub user_id: String,
    pub is_bot: bool,
    pub is_active: bool,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub last_refreshed: Option<String>,
}

/// Result of refreshing account
pub struct RefreshAccountResult {
    pub message: String,
}

/// Result of setting account type
pub struct SetAccountTypeResult {
    pub message: String,
}

/// Account command handlers
pub struct AccountCommands;

impl AccountCommands {
    /// Add a new account credential
    pub async fn add_account(
        client: &GrpcClient,
        platform_str: &str,
        typed_name: &str,
        is_bot: bool,
        is_broadcaster: bool,
        is_teammate: bool,
    ) -> Result<AddAccountResult, CommandError> {
        // Parse platform
        let platform = parse_platform(platform_str)?;
        
        // Begin auth flow
        let request = BeginAuthFlowRequest {
            platform: platform as i32,
            is_bot,
            redirect_uri: "http://127.0.0.1:9876".to_string(),
            requested_scopes: vec![],
        };
        
        let mut client = client.credential.clone();
        let response = client
            .begin_auth_flow(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        Ok(AddAccountResult {
            message: format!("Starting auth flow for {} account '{}'", platform_str, typed_name),
            auth_url: Some(response.auth_url),
            state: Some(response.state),
        })
    }
    
    /// Remove an account credential
    pub async fn remove_account(
        client: &GrpcClient,
        platform_str: &str,
        user_str: &str,
    ) -> Result<RemoveAccountResult, CommandError> {
        let platform = parse_platform(platform_str)?;
        
        // Try to parse as UUID first, otherwise treat as username
        let identifier = if let Ok(uuid) = uuid::Uuid::parse_str(user_str) {
            Some(maowbot_proto::maowbot::services::revoke_credential_request::Identifier::CredentialId(uuid.to_string()))
        } else {
            // For username, we'd need to look up the user first
            // For now, assume it's a user_id string
            Some(maowbot_proto::maowbot::services::revoke_credential_request::Identifier::PlatformUser(
                PlatformUserIdentifier {
                    platform: platform as i32,
                    user_id: user_str.to_string(),
                }
            ))
        };
        
        let request = RevokeCredentialRequest {
            identifier,
            revoke_at_platform: false,
        };
        
        let mut client = client.credential.clone();
        client
            .revoke_credential(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(RemoveAccountResult {
            message: format!("Removed credentials for platform='{}', user='{}'", platform_str, user_str),
        })
    }
    
    /// List account credentials
    pub async fn list_accounts(
        client: &GrpcClient,
        platform_str: Option<&str>,
    ) -> Result<ListAccountsResult, CommandError> {
        let platforms = if let Some(p) = platform_str {
            vec![parse_platform(p)? as i32]
        } else {
            vec![]
        };
        
        let request = ListCredentialsRequest {
            platforms,
            active_only: false,
            include_expired: true,
            page: None,
        };
        
        let mut client = client.credential.clone();
        let response = client
            .list_credentials(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        let credentials = response.credentials.into_iter().map(|info| {
            let cred = info.credential.unwrap_or_default();
            let user = info.user.unwrap_or_default();
            CredentialInfo {
                username: if user.global_username.is_empty() { 
                    cred.user_id.clone() 
                } else { 
                    user.global_username 
                },
                platform: platform_to_display_string(Platform::try_from(cred.platform).unwrap_or(Platform::Unknown)).to_string(),
                is_bot: cred.is_bot,
                credential_id: cred.credential_id,
                user_id: cred.user_id,
            }
        }).collect();
        
        Ok(ListAccountsResult { credentials })
    }
    
    /// Show account details
    pub async fn show_account(
        client: &GrpcClient,
        platform_str: &str,
        user_str: &str,
    ) -> Result<ShowAccountResult, CommandError> {
        let platform = parse_platform(platform_str)?;
        
        // List credentials for this platform and find matching user
        let request = ListCredentialsRequest {
            platforms: vec![platform as i32],
            active_only: false,
            include_expired: true,
            page: None,
        };
        
        let mut client = client.credential.clone();
        let response = client
            .list_credentials(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        // Try to find by user_id or username
        let credential_info = response.credentials.into_iter().find(|info| {
            if let Some(cred) = &info.credential {
                if cred.user_id == user_str {
                    return true;
                }
                if let Some(user) = &info.user {
                    if user.global_username == user_str {
                        return true;
                    }
                }
            }
            false
        });
        
        let credential = credential_info.map(|info| {
            let cred = info.credential.unwrap_or_default();
            CredentialDetail {
                platform: platform_to_display_string(Platform::try_from(cred.platform).unwrap_or(Platform::Unknown)).to_string(),
                user_id: cred.user_id,
                is_bot: cred.is_bot,
                is_active: cred.is_active,
                expires_at: cred.token_expires_at.map(|ts| format!("{:?}", ts)),
                created_at: cred.created_at.map(|ts| format!("{:?}", ts)).unwrap_or_default(),
                last_refreshed: cred.last_refreshed.map(|ts| format!("{:?}", ts)),
            }
        });
        
        Ok(ShowAccountResult { credential })
    }
    
    /// Refresh account credentials
    pub async fn refresh_account(
        client: &GrpcClient,
        platform_str: &str,
        user_str: &str,
    ) -> Result<RefreshAccountResult, CommandError> {
        let platform = parse_platform(platform_str)?;
        
        let identifier = Some(maowbot_proto::maowbot::services::refresh_credential_request::Identifier::PlatformUser(
            PlatformUserIdentifier {
                platform: platform as i32,
                user_id: user_str.to_string(),
            }
        ));
        
        let request = RefreshCredentialRequest {
            identifier,
            force_refresh: false,
        };
        
        let mut client = client.credential.clone();
        let response = client
            .refresh_credential(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        if response.was_refreshed {
            if let Some(cred) = response.credential {
                Ok(RefreshAccountResult {
                    message: format!(
                        "Successfully refreshed credential for platform={:?}, user_id={}, new expires_at={:?}",
                        Platform::try_from(cred.platform).unwrap_or(Platform::Unknown),
                        cred.user_id,
                        cred.token_expires_at
                    ),
                })
            } else {
                Ok(RefreshAccountResult {
                    message: "Credential refreshed but no details returned".to_string(),
                })
            }
        } else {
            Ok(RefreshAccountResult {
                message: if response.error_message.is_empty() { 
                    "Credential refresh not needed".to_string() 
                } else { 
                    response.error_message 
                },
            })
        }
    }
    
    /// Set account type (bot/broadcaster/teammate)
    pub async fn set_account_type(
        client: &GrpcClient,
        platform_str: &str,
        user_str: &str,
        is_bot: bool,
        _is_broadcaster: bool,
        _is_teammate: bool,
    ) -> Result<SetAccountTypeResult, CommandError> {
        // Note: The current gRPC API only supports updating is_bot flag
        // is_broadcaster and is_teammate are not available in the proto definition
        
        let platform = parse_platform(platform_str)?;
        
        // First, get the credential
        let list_request = ListCredentialsRequest {
            platforms: vec![platform as i32],
            active_only: false,
            include_expired: true,
            page: None,
        };
        
        let mut client_clone = client.credential.clone();
        let list_response = client_clone
            .list_credentials(list_request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let list_response = list_response.into_inner();
        
        // Find the credential
        let credential_info = list_response.credentials.into_iter().find(|info| {
            if let Some(cred) = &info.credential {
                if cred.user_id == user_str {
                    return true;
                }
                if let Some(user) = &info.user {
                    if user.global_username == user_str {
                        return true;
                    }
                }
            }
            false
        });
        
        if let Some(info) = credential_info {
            let mut cred = info.credential.unwrap_or_default();
            cred.is_bot = is_bot;
            
            let store_request = StoreCredentialRequest {
                credential: Some(cred),
                update_if_exists: true,
            };
            
            let mut client_store = client.credential.clone();
            client_store
                .store_credential(store_request)
                .await
                .map_err(|e| CommandError::GrpcError(e.to_string()))?;
                
            Ok(SetAccountTypeResult {
                message: format!("Updated credential: is_bot={}", is_bot),
            })
        } else {
            Err(CommandError::NotFound(format!(
                "No credential found for platform={}, user={}",
                platform_str, user_str
            )))
        }
    }
}

fn parse_platform(platform_str: &str) -> Result<Platform, CommandError> {
    match platform_str.to_lowercase().as_str() {
        "twitch" | "twitch-helix" | "twitchhelix" => Ok(Platform::TwitchHelix),
        "twitch-irc" | "twitchirc" => Ok(Platform::TwitchIrc),
        "twitch-eventsub" | "twitcheventsub" => Ok(Platform::TwitchEventsub),
        "discord" => Ok(Platform::Discord),
        "vrchat" => Ok(Platform::Vrchat),
        "vrchat-pipeline" | "vrchatpipeline" => Ok(Platform::VrchatPipeline),
        _ => Err(CommandError::InvalidInput(format!("Unknown platform '{}'", platform_str))),
    }
}