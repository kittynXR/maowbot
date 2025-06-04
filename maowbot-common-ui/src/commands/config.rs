use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    GetConfigRequest, SetConfigRequest, DeleteConfigRequest, ListConfigsRequest,
    ShutdownServerRequest,
};

/// Result of listing configs
pub struct ListConfigsResult {
    pub configs: Vec<ConfigInfo>,
}

pub struct ConfigInfo {
    pub key: String,
    pub value: String,
}

/// Result of setting a config
pub struct SetConfigResult {
    pub key: String,
    pub value: String,
    pub was_created: bool,
    pub previous_value: Option<String>,
}

/// Result of getting a config
pub struct GetConfigResult {
    pub key: String,
    pub value: String,
}

/// Result of server shutdown request
pub struct ShutdownResult {
    pub accepted: bool,
    pub message: String,
    pub shutdown_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Config command handlers
pub struct ConfigCommands;

impl ConfigCommands {
    /// List all configuration entries
    pub async fn list_configs(
        client: &GrpcClient,
    ) -> Result<ListConfigsResult, CommandError> {
        let request = ListConfigsRequest {
            categories: vec![],
            include_secrets: true,
            include_metadata: false,
            key_prefix: String::new(),
            page: None,
        };
        
        let mut client = client.config.clone();
        let response = client
            .list_configs(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        let configs = response.configs.into_iter().map(|entry| ConfigInfo {
            key: entry.key,
            value: entry.value,
        }).collect();
        
        Ok(ListConfigsResult { configs })
    }
    
    /// Get a single configuration value
    pub async fn get_config(
        client: &GrpcClient,
        key: &str,
    ) -> Result<GetConfigResult, CommandError> {
        let request = GetConfigRequest {
            key: key.to_string(),
            include_metadata: false,
        };
        
        let mut client = client.config.clone();
        let response = client
            .get_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        if let Some(config) = response.config {
            Ok(GetConfigResult {
                key: config.key,
                value: config.value,
            })
        } else {
            Err(CommandError::NotFound(format!("Config key '{}' not found", key)))
        }
    }
    
    /// Set a configuration value
    pub async fn set_config(
        client: &GrpcClient,
        key: &str,
        value: &str,
    ) -> Result<SetConfigResult, CommandError> {
        let request = SetConfigRequest {
            key: key.to_string(),
            value: value.to_string(),
            metadata: None,
            validate_only: false,
        };
        
        let mut client = client.config.clone();
        let response = client
            .set_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        Ok(SetConfigResult {
            key: key.to_string(),
            value: value.to_string(),
            was_created: response.was_created,
            previous_value: if response.previous_value.is_empty() {
                None
            } else {
                Some(response.previous_value)
            },
        })
    }
    
    /// Delete a configuration key
    pub async fn delete_config(
        client: &GrpcClient,
        key: &str,
    ) -> Result<(), CommandError> {
        let request = DeleteConfigRequest {
            key: key.to_string(),
        };
        
        let mut client = client.config.clone();
        client
            .delete_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Request server shutdown
    pub async fn shutdown_server(
        client: &GrpcClient,
        reason: Option<&str>,
        grace_period_seconds: Option<i32>,
    ) -> Result<ShutdownResult, CommandError> {
        let request = ShutdownServerRequest {
            reason: reason.unwrap_or("").to_string(),
            grace_period_seconds: grace_period_seconds.unwrap_or(30),
        };
        
        let mut client = client.config.clone();
        let response = client
            .shutdown_server(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        let shutdown_at = response.shutdown_at.and_then(|ts| {
            chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
        });
        
        Ok(ShutdownResult {
            accepted: response.accepted,
            message: response.message,
            shutdown_at,
        })
    }
}