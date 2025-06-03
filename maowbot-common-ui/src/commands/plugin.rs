use crate::GrpcClient;
use super::CommandError;
use maowbot_proto::maowbot::services::{
    ListPluginsRequest, EnablePluginRequest, DisablePluginRequest, RemovePluginRequest,
    GetSystemStatusRequest, PluginInfo, plugin_status,
};

/// Result of listing plugins
pub struct ListPluginsResult {
    pub plugins: Vec<PluginListItem>,
}

pub struct PluginListItem {
    pub name: String,
    pub version: String,
    pub state: PluginState,
    pub state_message: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug)]
pub enum PluginState {
    Unknown,
    Loaded,
    Running,
    Stopped,
    Error,
    Updating,
}

impl From<plugin_status::State> for PluginState {
    fn from(state: plugin_status::State) -> Self {
        match state {
            plugin_status::State::Unknown => PluginState::Unknown,
            plugin_status::State::Loaded => PluginState::Loaded,
            plugin_status::State::Running => PluginState::Running,
            plugin_status::State::Stopped => PluginState::Stopped,
            plugin_status::State::Error => PluginState::Error,
            plugin_status::State::Updating => PluginState::Updating,
        }
    }
}

/// Result of enabling a plugin
pub struct EnablePluginResult {
    pub plugin_name: String,
    pub state: PluginState,
}

/// Result of system status query
pub struct SystemStatusResult {
    pub total_plugins: u32,
    pub active_plugins: u32,
    pub uptime_seconds: i64,
    pub connected_plugins: Vec<String>,
}

/// Plugin command handlers
pub struct PluginCommands;

impl PluginCommands {
    /// List all plugins
    pub async fn list_plugins(
        client: &GrpcClient,
        active_only: bool,
    ) -> Result<ListPluginsResult, CommandError> {
        let request = ListPluginsRequest {
            active_only,
            include_system_plugins: true,
        };
        
        let mut client = client.plugin.clone();
        let response = client
            .list_plugins(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        let plugins = response.plugins.into_iter().map(|info| {
            let plugin = info.plugin.unwrap_or_default();
            let status = info.status.unwrap_or_default();
            
            PluginListItem {
                name: plugin.plugin_name,
                version: plugin.version,
                state: plugin_status::State::try_from(status.state)
                    .unwrap_or(plugin_status::State::Unknown)
                    .into(),
                state_message: if status.message.is_empty() { 
                    None 
                } else { 
                    Some(status.message) 
                },
                capabilities: info.granted_capabilities,
            }
        }).collect();
        
        Ok(ListPluginsResult { plugins })
    }
    
    /// Enable a plugin
    pub async fn enable_plugin(
        client: &GrpcClient,
        plugin_name: &str,
    ) -> Result<EnablePluginResult, CommandError> {
        let request = EnablePluginRequest {
            plugin_name: plugin_name.to_string(),
            startup_config: std::collections::HashMap::new(),
        };
        
        let mut client = client.plugin.clone();
        let response = client
            .enable_plugin(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        if let Some(info) = response.plugin {
            let status = info.status.unwrap_or_default();
            Ok(EnablePluginResult {
                plugin_name: plugin_name.to_string(),
                state: plugin_status::State::try_from(status.state)
                    .unwrap_or(plugin_status::State::Unknown)
                    .into(),
            })
        } else {
            Err(CommandError::DataError("No plugin info returned".to_string()))
        }
    }
    
    /// Disable a plugin
    pub async fn disable_plugin(
        client: &GrpcClient,
        plugin_name: &str,
    ) -> Result<(), CommandError> {
        let request = DisablePluginRequest {
            plugin_name: plugin_name.to_string(),
            force: false,
        };
        
        let mut client = client.plugin.clone();
        client
            .disable_plugin(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Remove a plugin
    pub async fn remove_plugin(
        client: &GrpcClient,
        plugin_name: &str,
    ) -> Result<(), CommandError> {
        let request = RemovePluginRequest {
            plugin_name: plugin_name.to_string(),
            remove_config: true,
            remove_data: false,
        };
        
        let mut client = client.plugin.clone();
        client
            .remove_plugin(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        Ok(())
    }
    
    /// Get system status including connected plugins
    pub async fn get_system_status(
        client: &GrpcClient,
    ) -> Result<SystemStatusResult, CommandError> {
        let request = GetSystemStatusRequest {
            include_metrics: false,
        };
        
        let mut client = client.plugin.clone();
        let response = client
            .get_system_status(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let response = response.into_inner();
        
        // Get list of connected (running) plugins
        let list_request = ListPluginsRequest {
            active_only: true,
            include_system_plugins: true,
        };
        
        let list_response = client
            .list_plugins(list_request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;
            
        let connected_plugins: Vec<String> = list_response
            .into_inner()
            .plugins
            .into_iter()
            .filter_map(|info| {
                let plugin = info.plugin?;
                let status = info.status?;
                if matches!(
                    plugin_status::State::try_from(status.state).ok()?,
                    plugin_status::State::Running
                ) {
                    Some(plugin.plugin_name)
                } else {
                    None
                }
            })
            .collect();
        
        Ok(SystemStatusResult {
            total_plugins: response.total_plugins as u32,
            active_plugins: response.active_plugins as u32,
            uptime_seconds: response.uptime_seconds,
            connected_plugins,
        })
    }
}