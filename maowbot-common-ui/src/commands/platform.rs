use crate::{GrpcClient, CommandResult, CommandError};
use maowbot_proto::maowbot::services::{
    CreatePlatformConfigRequest, GetPlatformConfigRequest, UpdatePlatformConfigRequest,
    DeletePlatformConfigRequest, ListPlatformConfigsRequest,
};
use maowbot_proto::maowbot::common::{PlatformConfig, PageRequest};

// Result structures
pub struct CreatePlatformConfigResult {
    pub config: PlatformConfig,
}

pub struct GetPlatformConfigResult {
    pub config: PlatformConfig,
}

pub struct UpdatePlatformConfigResult {
    pub config: PlatformConfig,
}

pub struct ListPlatformConfigsResult {
    pub configs: Vec<PlatformConfig>,
    pub total_count: i32,
}

// Command handlers
pub struct PlatformCommands;

impl PlatformCommands {
    pub async fn create_platform_config(
        client: &GrpcClient,
        platform: i32, // Use i32 directly
        client_id: &str,
        client_secret: Option<&str>,
        scopes: Vec<String>,
    ) -> Result<CommandResult<CreatePlatformConfigResult>, CommandError> {
        let request = CreatePlatformConfigRequest {
            platform,
            client_id: client_id.to_string(),
            client_secret: client_secret.unwrap_or_default().to_string(),
            scopes,
            additional_config: Default::default(),
        };

        let response = client.platform.clone()
            .create_platform_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let config = response.into_inner().config
            .ok_or_else(|| CommandError::DataError("No config returned".to_string()))?;

        Ok(CommandResult {
            data: CreatePlatformConfigResult { config },
            warnings: vec![],
        })
    }

    pub async fn get_platform_config(
        client: &GrpcClient,
        platform_config_id: &str,
    ) -> Result<CommandResult<GetPlatformConfigResult>, CommandError> {
        let request = GetPlatformConfigRequest {
            platform_config_id: platform_config_id.to_string(),
        };

        let response = client.platform.clone()
            .get_platform_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let config = response.into_inner().config
            .ok_or_else(|| CommandError::DataError("No config returned".to_string()))?;

        Ok(CommandResult {
            data: GetPlatformConfigResult { config },
            warnings: vec![],
        })
    }

    pub async fn update_platform_config(
        client: &GrpcClient,
        platform_config_id: &str,
        config: PlatformConfig,
    ) -> Result<CommandResult<UpdatePlatformConfigResult>, CommandError> {
        let request = UpdatePlatformConfigRequest {
            platform_config_id: platform_config_id.to_string(),
            config: Some(config),
            update_mask: None, // TODO: Add field mask support
        };

        let response = client.platform.clone()
            .update_platform_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let config = response.into_inner().config
            .ok_or_else(|| CommandError::DataError("No config returned".to_string()))?;

        Ok(CommandResult {
            data: UpdatePlatformConfigResult { config },
            warnings: vec![],
        })
    }

    pub async fn delete_platform_config(
        client: &GrpcClient,
        platform_config_id: &str,
    ) -> Result<CommandResult<()>, CommandError> {
        let request = DeletePlatformConfigRequest {
            platform_config_id: platform_config_id.to_string(),
        };

        client.platform.clone()
            .delete_platform_config(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: (),
            warnings: vec![],
        })
    }

    pub async fn list_platform_configs(
        client: &GrpcClient,
        platforms: Vec<i32>, // Use i32 directly
        page_size: i32,
    ) -> Result<CommandResult<ListPlatformConfigsResult>, CommandError> {
        let request = ListPlatformConfigsRequest {
            platforms,
            page: Some(PageRequest {
                page_size,
                page_token: String::new(),
            }),
        };

        let response = client.platform.clone()
            .list_platform_configs(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();
        let total_count = resp.page.as_ref()
            .map(|p| p.total_count)
            .unwrap_or(resp.configs.len() as i32);

        Ok(CommandResult {
            data: ListPlatformConfigsResult {
                configs: resp.configs,
                total_count,
            },
            warnings: vec![],
        })
    }
}