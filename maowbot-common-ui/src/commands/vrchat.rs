use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    GetCurrentWorldRequest, GetCurrentAvatarRequest, ChangeAvatarRequest,
    GetCurrentInstanceRequest, ListCredentialsRequest, SetConfigRequest,
};
use maowbot_proto::maowbot::common::Platform;

/// VRChat world information
pub struct VRChatWorldInfo {
    pub name: String,
    pub author_name: String,
    pub release_status: String,
    pub capacity: i32,
    pub created_at: String,
    pub updated_at: String,
    pub description: String,
}

/// VRChat avatar information
pub struct VRChatAvatarInfo {
    pub avatar_name: String,
    pub avatar_id: String,
}

/// VRChat instance information
pub struct VRChatInstanceInfo {
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub location: Option<String>,
}

/// VRChat command handlers
pub struct VRChatCommands;

impl VRChatCommands {
    /// Get current world
    pub async fn get_current_world(
        client: &GrpcClient,
        account_name: &str,
    ) -> Result<VRChatWorldInfo, CommandError> {
        let request = GetCurrentWorldRequest {
            account_name: account_name.to_string(),
        };
        
        let mut vrchat_client = client.vrchat.clone();
        let response = vrchat_client
            .get_current_world(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let world = response
            .into_inner()
            .world
            .ok_or_else(|| CommandError::NotFound("No world data found".to_string()))?;
            
        Ok(VRChatWorldInfo {
            name: world.name.clone(),
            author_name: world.author_name.clone(),
            release_status: format!("{:?}", world.release_status()),
            capacity: world.capacity,
            created_at: world.created_at.map(|t| format!("{}", t.seconds)).unwrap_or_default(),
            updated_at: world.updated_at.map(|t| format!("{}", t.seconds)).unwrap_or_default(),
            description: world.description,
        })
    }
    
    /// Get current avatar
    pub async fn get_current_avatar(
        client: &GrpcClient,
        account_name: &str,
    ) -> Result<VRChatAvatarInfo, CommandError> {
        let request = GetCurrentAvatarRequest {
            account_name: account_name.to_string(),
        };
        
        let mut vrchat_client = client.vrchat.clone();
        let response = vrchat_client
            .get_current_avatar(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let avatar = response
            .into_inner()
            .avatar
            .ok_or_else(|| CommandError::NotFound("No avatar data found".to_string()))?;
            
        Ok(VRChatAvatarInfo {
            avatar_name: avatar.name,
            avatar_id: avatar.avatar_id,
        })
    }
    
    /// Change avatar
    pub async fn change_avatar(
        client: &GrpcClient,
        account_name: &str,
        avatar_id: &str,
    ) -> Result<(), CommandError> {
        let request = ChangeAvatarRequest {
            account_name: account_name.to_string(),
            avatar_id: avatar_id.to_string(),
        };
        
        let mut vrchat_client = client.vrchat.clone();
        let response = vrchat_client
            .change_avatar(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let resp = response.into_inner();
        if !resp.success {
            return Err(CommandError::DataError(resp.error_message));
        }
        
        Ok(())
    }
    
    /// Get current instance
    pub async fn get_current_instance(
        client: &GrpcClient,
        account_name: &str,
    ) -> Result<VRChatInstanceInfo, CommandError> {
        let request = GetCurrentInstanceRequest {
            account_name: account_name.to_string(),
        };
        
        let mut vrchat_client = client.vrchat.clone();
        let response = vrchat_client
            .get_current_instance(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let instance = response.into_inner().instance;
        
        if let Some(inst) = instance {
            let world_id = inst.world_id.clone();
            let instance_id = inst.instance_id.clone();
            Ok(VRChatInstanceInfo {
                world_id: Some(world_id.clone()),
                instance_id: Some(instance_id.clone()),
                location: Some(format!("{}:{}", world_id, instance_id)),
            })
        } else {
            Ok(VRChatInstanceInfo {
                world_id: None,
                instance_id: None,
                location: None,
            })
        }
    }
    
    /// Set active VRChat account
    pub async fn set_vrchat_account(
        client: &GrpcClient,
        account_name: &str,
    ) -> Result<(), CommandError> {
        // Verify we have a VRChat credential for this account
        let request = ListCredentialsRequest {
            platforms: vec![Platform::Vrchat as i32],
            active_only: true,
            include_expired: false,
            page: None,
        };
        
        let mut cred_client = client.credential.clone();
        let response = cred_client
            .list_credentials(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let credentials = response.into_inner().credentials;
        let found = credentials
            .iter()
            .any(|c| c.credential.as_ref()
                .map(|cred| cred.user_name.eq_ignore_ascii_case(account_name))
                .unwrap_or(false));
            
        if !found {
            return Err(CommandError::NotFound(format!(
                "No VRChat credential found with user_name='{}'. Try 'account add vrchat' first.",
                account_name
            )));
        }
        
        // Store in config
        let config_request = SetConfigRequest {
            key: "vrchat_active_account".to_string(),
            value: account_name.to_string(),
            metadata: None,
            validate_only: false,
        };
        
        let mut config_client = client.config.clone();
        config_client
            .set_config(config_request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
}