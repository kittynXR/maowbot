use tonic::{Request, Response, Status};
use prost_types;
use maowbot_proto::maowbot::{
    common::{Platform, PlatformCredential},
    services::{
        credential_service_server::CredentialService,
        RefreshResult, PlatformCredentials, PlatformHealth, OverallHealth,
        *,
    },
};
use maowbot_core::{
    auth::manager::AuthManager,
    repositories::postgres::credentials::PostgresCredentialsRepository,
};
use tokio::sync::Mutex;
use maowbot_common::models::platform::PlatformCredential as Credential;
use maowbot_common::traits::repository_traits::CredentialsRepository;
use std::sync::Arc;
use std::str::FromStr;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, error, debug};

pub struct CredentialServiceImpl {
    auth_manager: Arc<Mutex<AuthManager>>,
    credential_repo: Arc<PostgresCredentialsRepository>,
}

impl CredentialServiceImpl {
    pub fn new(
        auth_manager: Arc<Mutex<AuthManager>>,
        credential_repo: Arc<PostgresCredentialsRepository>,
    ) -> Self {
        Self {
            auth_manager,
            credential_repo,
        }
    }
    
    fn credential_to_proto(cred: &Credential) -> PlatformCredential {
        PlatformCredential {
            credential_id: cred.credential_id.to_string(),
            platform: match cred.platform {
                maowbot_common::models::platform::Platform::TwitchIRC => Platform::TwitchIrc as i32,
                maowbot_common::models::platform::Platform::TwitchEventSub => Platform::TwitchEventsub as i32,
                maowbot_common::models::platform::Platform::Discord => Platform::Discord as i32,
                maowbot_common::models::platform::Platform::VRChat => Platform::Vrchat as i32,
                maowbot_common::models::platform::Platform::Twitch => Platform::TwitchHelix as i32,
                _ => Platform::Unknown as i32,
            },
            user_id: cred.user_id.to_string(),
            user_name: cred.user_name.clone(),
            display_name: String::new(), // We don't have display_name in the model
            encrypted_access_token: String::new(), // Don't expose encrypted tokens
            encrypted_refresh_token: String::new(), // Don't expose encrypted tokens
            token_expires_at: cred.expires_at.map(|ts| prost_types::Timestamp {
                seconds: ts.timestamp(),
                nanos: ts.timestamp_subsec_nanos() as i32,
            }),
            scopes: vec![], // We don't have scopes in the model
            created_at: Some(prost_types::Timestamp {
                seconds: cred.created_at.timestamp(),
                nanos: cred.created_at.timestamp_subsec_nanos() as i32,
            }),
            last_refreshed: Some(prost_types::Timestamp {
                seconds: cred.updated_at.timestamp(),
                nanos: cred.updated_at.timestamp_subsec_nanos() as i32,
            }),
            is_active: true, // We don't have is_active in the model
            is_bot: cred.is_bot,
            is_broadcaster: cred.is_broadcaster,
            is_teammate: cred.is_teammate,
        }
    }
}

#[tonic::async_trait]
impl CredentialService for CredentialServiceImpl {
    async fn begin_auth_flow(
        &self,
        request: Request<BeginAuthFlowRequest>,
    ) -> Result<Response<BeginAuthFlowResponse>, Status> {
        let req = request.into_inner();
        let platform = Platform::try_from(req.platform)
            .map_err(|_| Status::invalid_argument("Invalid platform"))?;
        
        info!("Beginning auth flow for platform: {:?}", platform);
        
        // Convert to internal platform enum
        let platform_str = match platform {
            Platform::TwitchIrc => "twitch-irc",
            Platform::TwitchEventsub => "twitch-eventsub",
            Platform::Discord => "discord",
            Platform::Vrchat => "vrchat",
            Platform::TwitchHelix => "twitch-helix",
            _ => return Err(Status::invalid_argument("Unsupported platform")),
        };
        
        let auth_url = self.auth_manager
            .lock()
            .await
            .begin_auth_flow(
                maowbot_common::models::platform::Platform::from_str(platform_str).unwrap(),
                req.is_bot,
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to begin auth flow: {}", e)))?;
        
        Ok(Response::new(BeginAuthFlowResponse {
            auth_url,
            state: String::new(), // TODO: Add state tracking
            code_verifier: String::new(), // TODO: PKCE support
            expires_at: None,
            metadata: Default::default(),
        }))
    }
    
    async fn complete_auth_flow(
        &self,
        request: Request<CompleteAuthFlowRequest>,
    ) -> Result<Response<CompleteAuthFlowResponse>, Status> {
        let req = request.into_inner();
        let platform = Platform::try_from(req.platform)
            .map_err(|_| Status::invalid_argument("Invalid platform"))?;
        
        info!("Completing auth flow for platform: {:?}", platform);
        
        let platform_str = match platform {
            Platform::TwitchIrc => "twitch-irc",
            Platform::TwitchEventsub => "twitch-eventsub",
            Platform::Discord => "discord",
            Platform::Vrchat => "vrchat",
            Platform::TwitchHelix => "twitch-helix",
            _ => return Err(Status::invalid_argument("Unsupported platform")),
        };
        
        let platform_internal = maowbot_common::models::platform::Platform::from_str(platform_str)
            .map_err(|_| Status::invalid_argument("Invalid platform"))?;
        
        // Handle different auth data types
        match req.auth_data {
            Some(complete_auth_flow_request::AuthData::OauthCode(oauth_data)) => {
                let user_id = Uuid::parse_str(&oauth_data.user_id)
                    .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
                
                let credential = self.auth_manager
                    .lock()
                    .await
                    .complete_auth_flow_for_user(
                        platform_internal,
                        oauth_data.code,
                        &user_id.to_string(),
                    )
                    .await
                    .map_err(|e| Status::internal(format!("Failed to complete auth flow: {}", e)))?;
                
                Ok(Response::new(CompleteAuthFlowResponse {
                    credential: Some(Self::credential_to_proto(&credential)),
                    requires_2fa: false,
                    session_token: String::new(),
                }))
            }
            Some(complete_auth_flow_request::AuthData::CredentialsMap(creds_data)) => {
                let user_id = Uuid::parse_str(&creds_data.user_id)
                    .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
                
                // Convert credentials map to the format expected by auth manager
                let creds_map: HashMap<String, String> = creds_data.credentials.into_iter().collect();
                
                let credential = self.auth_manager
                    .lock()
                    .await
                    .complete_auth_flow_for_user_multi(
                        platform_internal,
                        &user_id,
                        creds_map,
                    )
                    .await
                    .map_err(|e| {
                        // Check if this is a 2FA prompt
                        let error_msg = e.to_string();
                        if error_msg.contains("__2FA_PROMPT__") {
                            // Return a special error that the client can handle
                            Status::unauthenticated(error_msg)
                        } else {
                            Status::internal(format!("Failed to complete auth flow: {}", e))
                        }
                    })?;
                
                Ok(Response::new(CompleteAuthFlowResponse {
                    credential: Some(Self::credential_to_proto(&credential)),
                    requires_2fa: false,
                    session_token: String::new(),
                }))
            }
            Some(complete_auth_flow_request::AuthData::TwoFactorCode(twofa_data)) => {
                let user_id = Uuid::parse_str(&twofa_data.user_id)
                    .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
                
                let credential = self.auth_manager
                    .lock()
                    .await
                    .complete_auth_flow_for_user_twofactor(
                        platform_internal,
                        twofa_data.code,
                        &user_id,
                    )
                    .await
                    .map_err(|e| Status::internal(format!("Failed to complete 2FA: {}", e)))?;
                
                Ok(Response::new(CompleteAuthFlowResponse {
                    credential: Some(Self::credential_to_proto(&credential)),
                    requires_2fa: false,
                    session_token: String::new(),
                }))
            }
            None => Err(Status::invalid_argument("Missing auth data")),
        }
    }
    
    async fn list_credentials(
        &self,
        request: Request<ListCredentialsRequest>,
    ) -> Result<Response<ListCredentialsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing credentials");
        
        let credentials = if req.platforms.is_empty() {
            self.credential_repo
                .get_all_credentials()
                .await
                .map_err(|e| Status::internal(format!("Failed to list credentials: {}", e)))?
        } else {
            // TODO: Filter by platforms
            self.credential_repo
                .get_all_credentials()
                .await
                .map_err(|e| Status::internal(format!("Failed to list credentials: {}", e)))?
        };
        
        let credential_infos: Vec<CredentialInfo> = credentials.into_iter()
            .map(|c| {
                let status = if c.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false) {
                    CredentialStatus::Expired
                } else {
                    CredentialStatus::Active
                };
                
                CredentialInfo {
                    credential: Some(Self::credential_to_proto(&c)),
                    status: status as i32,
                    user: None, // TODO: Populate user if requested
                }
            })
            .collect();
        
        Ok(Response::new(ListCredentialsResponse {
            credentials: credential_infos,
            page: None,
        }))
    }
    
    async fn refresh_credential(
        &self,
        request: Request<RefreshCredentialRequest>,
    ) -> Result<Response<RefreshCredentialResponse>, Status> {
        let req = request.into_inner();
        
        let credential_id = match req.identifier {
            Some(refresh_credential_request::Identifier::CredentialId(id)) => {
                Uuid::parse_str(&id)
                    .map_err(|e| Status::invalid_argument(format!("Invalid credential_id: {}", e)))?
            }
            _ => return Err(Status::unimplemented("Platform user identifier not yet supported")),
        };
        
        info!("Refreshing credential: {}", credential_id);
        
        // Get the credential
        let credential = self.credential_repo
            .get_credential_by_id(credential_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get credential: {}", e)))?
            .ok_or_else(|| Status::not_found("Credential not found"))?;
        
        // Refresh it
        let mut auth_guard = self.auth_manager.lock().await;
        let refreshed = auth_guard
            .refresh_platform_credentials(&credential.platform, &credential.user_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to refresh credential: {}", e)))?;
        
        Ok(Response::new(RefreshCredentialResponse {
            credential: Some(Self::credential_to_proto(&refreshed)),
            was_refreshed: true,
            error_message: String::new(),
        }))
    }
    
    async fn complete_auth_flow2_fa(
        &self,
        _request: Request<CompleteAuthFlow2FaRequest>,
    ) -> Result<Response<CompleteAuthFlowResponse>, Status> {
        Err(Status::unimplemented("2FA not yet implemented"))
    }
    
    async fn get_credential(
        &self,
        request: Request<GetCredentialRequest>,
    ) -> Result<Response<GetCredentialResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting credential by ID: {}", req.credential_id);
        
        let credential_id = Uuid::parse_str(&req.credential_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid credential_id: {}", e)))?;
        
        let credential = self.credential_repo
            .get_credential_by_id(credential_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get credential: {}", e)))?
            .ok_or_else(|| Status::not_found("Credential not found"))?;
        
        let status = if credential.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false) {
            CredentialStatus::Expired
        } else {
            CredentialStatus::Active
        };
        
        Ok(Response::new(GetCredentialResponse {
            credential: Some(CredentialInfo {
                credential: Some(Self::credential_to_proto(&credential)),
                status: status as i32,
                user: None, // TODO: Include user if requested
            }),
        }))
    }
    
    async fn store_credential(
        &self,
        request: Request<StoreCredentialRequest>,
    ) -> Result<Response<StoreCredentialResponse>, Status> {
        let req = request.into_inner();
        let cred_proto = req.credential.ok_or_else(|| Status::invalid_argument("Missing credential"))?;
        
        info!("Storing credential for platform {:?}", cred_proto.platform);
        
        let platform = Platform::try_from(cred_proto.platform)
            .map_err(|_| Status::invalid_argument("Invalid platform"))?;
        
        let platform_internal = match platform {
            Platform::TwitchIrc => maowbot_common::models::platform::Platform::TwitchIRC,
            Platform::TwitchEventsub => maowbot_common::models::platform::Platform::TwitchEventSub,
            Platform::Discord => maowbot_common::models::platform::Platform::Discord,
            Platform::Vrchat => maowbot_common::models::platform::Platform::VRChat,
            Platform::TwitchHelix => maowbot_common::models::platform::Platform::Twitch,
            _ => return Err(Status::invalid_argument("Unsupported platform")),
        };
        
        let user_id = Uuid::parse_str(&cred_proto.user_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid user_id: {}", e)))?;
        
        let credential_id = if cred_proto.credential_id.is_empty() {
            Uuid::new_v4()
        } else {
            Uuid::parse_str(&cred_proto.credential_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid credential_id: {}", e)))?
        };
        
        // Get the existing credential if updating
        let existing = if req.update_if_exists {
            self.credential_repo
                .get_credential_by_id(credential_id)
                .await
                .map_err(|e| Status::internal(format!("Failed to get existing credential: {}", e)))?
        } else {
            None
        };
        
        if let Some(mut existing_cred) = existing {
            // Update only the flags
            existing_cred.is_bot = cred_proto.is_bot;
            existing_cred.is_broadcaster = cred_proto.is_broadcaster;
            existing_cred.is_teammate = cred_proto.is_teammate;
            existing_cred.updated_at = Utc::now();
            
            self.credential_repo
                .store_credentials(&existing_cred)
                .await
                .map_err(|e| Status::internal(format!("Failed to update credential: {}", e)))?;
                
            Ok(Response::new(StoreCredentialResponse {
                credential: Some(Self::credential_to_proto(&existing_cred)),
                was_updated: true,
            }))
        } else {
            // For new credentials, we can't really create them from scratch
            // They should be created through the auth flow
            Err(Status::invalid_argument("Cannot create new credentials through store_credential. Use auth flow instead."))
        }
    }
    
    async fn revoke_credential(
        &self,
        request: Request<RevokeCredentialRequest>,
    ) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Revoking credential");
        
        let (platform, user_id) = match req.identifier {
            Some(revoke_credential_request::Identifier::CredentialId(id)) => {
                let credential_id = Uuid::parse_str(&id)
                    .map_err(|e| Status::invalid_argument(format!("Invalid credential_id: {}", e)))?;
                
                let credential = self.credential_repo
                    .get_credential_by_id(credential_id)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to get credential: {}", e)))?
                    .ok_or_else(|| Status::not_found("Credential not found"))?;
                
                (credential.platform, credential.user_id)
            }
            _ => return Err(Status::unimplemented("Platform user identifier not yet supported")),
        };
        
        self.credential_repo
            .delete_credentials(&platform, user_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to revoke credential: {}", e)))?;
        
        Ok(Response::new(()))
    }
    
    async fn batch_refresh_credentials(
        &self,
        request: Request<BatchRefreshCredentialsRequest>,
    ) -> Result<Response<BatchRefreshCredentialsResponse>, Status> {
        let req = request.into_inner();
        info!("Batch refreshing {} credentials", req.credential_ids.len());
        
        let mut results = Vec::new();
        
        for credential_id_str in &req.credential_ids {
            let result = match Uuid::parse_str(credential_id_str) {
                Ok(credential_id) => {
                    match self.credential_repo.get_credential_by_id(credential_id).await {
                        Ok(Some(credential)) => {
                            let mut auth_guard = self.auth_manager.lock().await;
                            match auth_guard.refresh_platform_credentials(&credential.platform, &credential.user_id).await {
                                Ok(refreshed) => RefreshResult {
                                    credential_id: credential_id_str.clone(),
                                    success: true,
                                    credential: Some(Self::credential_to_proto(&refreshed)),
                                    error_message: String::new(),
                                },
                                Err(e) => RefreshResult {
                                    credential_id: credential_id_str.clone(),
                                    success: false,
                                    credential: None,
                                    error_message: format!("Failed to refresh: {}", e),
                                },
                            }
                        }
                        Ok(None) => RefreshResult {
                            credential_id: credential_id_str.clone(),
                            success: false,
                            credential: None,
                            error_message: "Credential not found".to_string(),
                        },
                        Err(e) => RefreshResult {
                            credential_id: credential_id_str.clone(),
                            success: false,
                            credential: None,
                            error_message: format!("Failed to get credential: {}", e),
                        },
                    }
                }
                Err(e) => RefreshResult {
                    credential_id: credential_id_str.clone(),
                    success: false,
                    credential: None,
                    error_message: format!("Invalid credential ID: {}", e),
                },
            };
            
            results.push(result);
        }
        
        let success_count = results.iter().filter(|r| r.success).count() as i32;
        let failure_count = results.len() as i32 - success_count;
        
        Ok(Response::new(BatchRefreshCredentialsResponse {
            results,
            success_count,
            failure_count,
        }))
    }
    
    async fn batch_list_credentials(
        &self,
        request: Request<BatchListCredentialsRequest>,
    ) -> Result<Response<BatchListCredentialsResponse>, Status> {
        let req = request.into_inner();
        debug!("Batch listing credentials");
        
        let platforms = if req.platforms.is_empty() {
            vec![
                maowbot_common::models::platform::Platform::TwitchIRC,
                maowbot_common::models::platform::Platform::TwitchEventSub,
                maowbot_common::models::platform::Platform::Discord,
                maowbot_common::models::platform::Platform::VRChat,
            ]
        } else {
            req.platforms.iter()
                .filter_map(|&p| {
                    match Platform::try_from(p) {
                        Ok(Platform::TwitchIrc) => Some(maowbot_common::models::platform::Platform::TwitchIRC),
                        Ok(Platform::TwitchEventsub) => Some(maowbot_common::models::platform::Platform::TwitchEventSub),
                        Ok(Platform::Discord) => Some(maowbot_common::models::platform::Platform::Discord),
                        Ok(Platform::Vrchat) => Some(maowbot_common::models::platform::Platform::VRChat),
                        _ => None,
                    }
                })
                .collect()
        };
        
        let mut all_credentials = Vec::new();
        let mut by_platform = std::collections::HashMap::new();
        
        for platform in platforms {
            let creds = self.credential_repo
                .list_credentials_for_platform(&platform)
                .await
                .map_err(|e| Status::internal(format!("Failed to list credentials: {}", e)))?;
            
            let mut platform_creds = Vec::new();
            let mut active_count = 0;
            let mut expired_count = 0;
            
            for cred in creds {
                let status = if cred.expires_at.map(|exp| exp < Utc::now()).unwrap_or(false) {
                    expired_count += 1;
                    CredentialStatus::Expired
                } else {
                    active_count += 1;
                    CredentialStatus::Active
                };
                
                let cred_info = CredentialInfo {
                    credential: Some(Self::credential_to_proto(&cred)),
                    status: status as i32,
                    user: None,
                };
                
                platform_creds.push(cred_info.clone());
                all_credentials.push(cred_info);
            }
            
            if req.group_by_platform {
                let platform_proto = match platform {
                    maowbot_common::models::platform::Platform::TwitchIRC => Platform::TwitchIrc,
                    maowbot_common::models::platform::Platform::TwitchEventSub => Platform::TwitchEventsub,
                    maowbot_common::models::platform::Platform::Discord => Platform::Discord,
                    maowbot_common::models::platform::Platform::VRChat => Platform::Vrchat,
                    _ => Platform::Unknown,
                };
                
                by_platform.insert(
                    platform.to_string(),
                    PlatformCredentials {
                        platform: platform_proto as i32,
                        credentials: platform_creds,
                        active_count,
                        expired_count,
                    },
                );
            }
        }
        
        Ok(Response::new(BatchListCredentialsResponse {
            by_platform,
            all_credentials,
        }))
    }
    
    async fn get_credential_health(
        &self,
        request: Request<GetCredentialHealthRequest>,
    ) -> Result<Response<GetCredentialHealthResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting credential health");
        
        let platforms = if req.platforms.is_empty() {
            vec![
                maowbot_common::models::platform::Platform::TwitchIRC,
                maowbot_common::models::platform::Platform::TwitchEventSub,
                maowbot_common::models::platform::Platform::Discord,
                maowbot_common::models::platform::Platform::VRChat,
            ]
        } else {
            req.platforms.iter()
                .filter_map(|&p| {
                    match Platform::try_from(p) {
                        Ok(Platform::TwitchIrc) => Some(maowbot_common::models::platform::Platform::TwitchIRC),
                        Ok(Platform::TwitchEventsub) => Some(maowbot_common::models::platform::Platform::TwitchEventSub),
                        Ok(Platform::Discord) => Some(maowbot_common::models::platform::Platform::Discord),
                        Ok(Platform::Vrchat) => Some(maowbot_common::models::platform::Platform::VRChat),
                        _ => None,
                    }
                })
                .collect()
        };
        
        let mut platform_health_list = Vec::new();
        let mut total_credentials = 0;
        let mut total_platforms = 0;
        let mut healthy_platforms = 0;
        
        for platform in platforms {
            let creds = self.credential_repo
                .list_credentials_for_platform(&platform)
                .await
                .map_err(|e| Status::internal(format!("Failed to list credentials: {}", e)))?;
            
            if creds.is_empty() {
                continue;
            }
            
            total_platforms += 1;
            let mut active_credentials = 0;
            let mut expired_credentials = 0;
            let mut expiring_soon = 0;
            let mut oldest_refresh = None;
            let mut newest_refresh = None;
            
            let now = Utc::now();
            
            for cred in &creds {
                total_credentials += 1;
                
                if cred.expires_at.map(|exp| exp < now).unwrap_or(false) {
                    expired_credentials += 1;
                } else {
                    active_credentials += 1;
                    if cred.expires_at.map(|exp| exp < now + chrono::Duration::hours(24)).unwrap_or(false) {
                        expiring_soon += 1;
                    }
                }
                
                if oldest_refresh.is_none() || cred.updated_at < oldest_refresh.unwrap() {
                    oldest_refresh = Some(cred.updated_at);
                }
                if newest_refresh.is_none() || cred.updated_at > newest_refresh.unwrap() {
                    newest_refresh = Some(cred.updated_at);
                }
            }
            
            if expired_credentials == 0 {
                healthy_platforms += 1;
            }
            
            let platform_proto = match platform {
                maowbot_common::models::platform::Platform::TwitchIRC => Platform::TwitchIrc,
                maowbot_common::models::platform::Platform::TwitchEventSub => Platform::TwitchEventsub,
                maowbot_common::models::platform::Platform::Discord => Platform::Discord,
                maowbot_common::models::platform::Platform::VRChat => Platform::Vrchat,
                _ => Platform::Unknown,
            };
            
            platform_health_list.push(PlatformHealth {
                platform: platform_proto as i32,
                total_credentials: creds.len() as i32,
                active_credentials,
                expired_credentials,
                expiring_soon,
                oldest_refresh: oldest_refresh.map(|ts| prost_types::Timestamp {
                    seconds: ts.timestamp(),
                    nanos: ts.timestamp_subsec_nanos() as i32,
                }),
                newest_refresh: newest_refresh.map(|ts| prost_types::Timestamp {
                    seconds: ts.timestamp(),
                    nanos: ts.timestamp_subsec_nanos() as i32,
                }),
            });
        }
        
        let health_score = if total_platforms > 0 {
            healthy_platforms as f32 / total_platforms as f32
        } else {
            0.0
        };
        
        Ok(Response::new(GetCredentialHealthResponse {
            platform_health: platform_health_list,
            overall: Some(OverallHealth {
                total_platforms,
                healthy_platforms,
                total_credentials,
                health_score,
            }),
        }))
    }
    
    type StreamCredentialUpdatesStream = tonic::codec::Streaming<CredentialUpdateEvent>;
    
    async fn stream_credential_updates(
        &self,
        _request: Request<StreamCredentialUpdatesRequest>,
    ) -> Result<Response<Self::StreamCredentialUpdatesStream>, Status> {
        Err(Status::unimplemented("stream_credential_updates not implemented"))
    }
}