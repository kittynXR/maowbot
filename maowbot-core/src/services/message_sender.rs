use std::sync::Arc;
use tracing::{debug, warn, info};
use uuid::Uuid;
use maowbot_common::models::platform::Platform;
use maowbot_common::models::platform::Platform::TwitchIRC;
use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::traits::repository_traits::CredentialsRepository;
use crate::platforms::manager::PlatformManager;
use crate::Error;

/// Generic response type for message sending operations
#[derive(Debug, Clone)]
pub struct MessageResponse {
    pub texts: Vec<String>,
    pub respond_credential_id: Option<Uuid>,
    pub platform: String,
    pub channel: String,
}

/// Service for sending messages across different platforms with proper credential selection
pub struct MessageSender {
    pub credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    pub platform_manager: Arc<PlatformManager>,
}

impl MessageSender {
    /// Create a new MessageSender service
    pub fn new(
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
        platform_manager: Arc<PlatformManager>,
    ) -> Self {
        Self {
            credentials_repo,
            platform_manager,
        }
    }

    /// Determine which credential to use for sending messages on a given platform
    /// 
    /// Follows these rules:
    /// 1. If specified_credential_id is provided and valid, use that
    /// 2. Find the first bot credential for the platform
    /// 3. Find the first broadcaster credential for the platform
    /// 4. Use the specified message_sender_user_id's credential if available
    /// 5. Return None if no suitable credential is found
    pub async fn select_response_credential(
        &self,
        platform: &Platform,
        specified_credential_id: Option<Uuid>,
        message_sender_user_id: Uuid,
    ) -> Result<Option<PlatformCredential>, Error> {
        // #1: if a specific credential ID is specified, try to use it first
        if let Some(cid) = specified_credential_id {
            if let Ok(Some(c)) = self.credentials_repo.get_credential_by_id(cid).await {
                if c.platform == *platform {
                    debug!("Using specified credential: {} ({})", c.user_name, c.credential_id);
                    return Ok(Some(c));
                }
            }
        }

        // #2: Get all credentials for this platform
        let all_creds = self.credentials_repo.list_credentials_for_platform(platform).await?;
        
        // If no credentials exist for this platform, return None
        if all_creds.is_empty() {
            warn!("No credentials found for platform {:?}", platform);
            return Ok(None);
        }

        // #3: Find the first bot credential
        if let Some(bot_cred) = all_creds.iter().find(|c| c.is_bot) {
            debug!("Using bot credential: {} ({})", bot_cred.user_name, bot_cred.credential_id);
            return Ok(Some(bot_cred.clone()));
        }

        // #4: Find the first broadcaster credential
        if let Some(broadcaster_cred) = all_creds.iter().find(|c| c.is_broadcaster) {
            debug!("Using broadcaster credential: {} ({})", broadcaster_cred.user_name, broadcaster_cred.credential_id);
            return Ok(Some(broadcaster_cred.clone()));
        }

        // #5: Try to use the message sender's own credential
        let maybe_same_user_cred = self.credentials_repo.get_credentials(
            platform,
            message_sender_user_id
        ).await?;
        
        if let Some(c) = maybe_same_user_cred {
            debug!("Using message sender's credential: {} ({})", c.user_name, c.credential_id);
            return Ok(Some(c));
        }

        // #6: If nothing else works, use the first credential we found
        if !all_creds.is_empty() {
            debug!("Using first available credential: {} ({})", all_creds[0].user_name, all_creds[0].credential_id);
            return Ok(Some(all_creds[0].clone()));
        }

        // If we can't find any suitable credential, just return None
        warn!("No suitable credential found for platform {:?}", platform);
        Ok(None)
    }

    /// Send a message to Twitch IRC
    pub async fn send_twitch_message(
        &self,
        channel: &str,
        message: &str,
        specified_credential_id: Option<Uuid>,
        message_sender_user_id: Uuid,
    ) -> Result<(), Error> {
        info!("Attempting to send Twitch message to channel: {}", channel);
        
        // Make sure the channel name starts with a # prefix for Twitch IRC
        let channel_with_hash = if !channel.starts_with('#') {
            format!("#{}", channel)
        } else {
            channel.to_string()
        };
        
        // Find a suitable credential for sending this message
        let credential_opt = self.select_response_credential(&TwitchIRC, specified_credential_id, message_sender_user_id).await?;
        
        match credential_opt {
            Some(credential) => {
                info!("Sending message using credential: {} to channel: {}", 
                      credential.user_name, channel_with_hash);
                
                self.platform_manager.send_twitch_irc_message(
                    &credential.user_name,
                    &channel_with_hash,
                    message
                ).await
            },
            None => {
                let err_msg = format!("No credential available to send Twitch IRC message to {}", channel);
                warn!("{}", err_msg);
                Err(Error::Internal(err_msg))
            }
        }
    }

    /// Send a response consisting of multiple message lines
    pub async fn send_response(
        &self,
        response: &MessageResponse,
        message_sender_user_id: Uuid,
    ) -> Result<(), Error> {
        match response.platform.as_str() {
            "twitch-irc" => {
                for text in &response.texts {
                    if let Err(e) = self.send_twitch_message(
                        &response.channel,
                        text,
                        response.respond_credential_id,
                        message_sender_user_id
                    ).await {
                        warn!("Error sending message: {:?}", e);
                    }
                }
                Ok(())
            },
            // Add more platforms as needed
            _ => {
                Err(Error::Internal(format!(
                    "Platform '{}' not supported for message sending",
                    response.platform
                )))
            }
        }
    }
}