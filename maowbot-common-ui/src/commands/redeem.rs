use crate::{GrpcClient, CommandResult, CommandError};
use maowbot_proto::maowbot::services::{
    CreateRedeemRequest, GetRedeemRequest, UpdateRedeemRequest,
    DeleteRedeemRequest, ListRedeemsRequest, ExecuteRedeemRequest,
    GetRedeemUsageRequest, SyncRedeemsRequest, RedeemInfo,
};
use maowbot_proto::maowbot::common::{Redeem, PageRequest};
use uuid::Uuid;

// Result structures
pub struct CreateRedeemResult {
    pub redeem: Redeem,
}

pub struct GetRedeemResult {
    pub redeem: Redeem,
}

pub struct UpdateRedeemResult {
    pub redeem: Redeem,
}

pub struct ListRedeemsResult {
    pub redeems: Vec<RedeemInfo>,
    pub total_count: i32,
}

pub struct ExecuteRedeemResult {
    pub output: String,
    pub success: bool,
}

pub struct GetRedeemUsageResult {
    pub total_uses: i64,
    pub unique_users: i64,
}

pub struct SyncRedeemsResult {
    pub added_count: i32,
    pub updated_count: i32,
    pub removed_count: i32,
}

// Command handlers
pub struct RedeemCommands;

impl RedeemCommands {
    pub async fn create_redeem(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
        twitch_id: Option<&str>,
        plugin_id: Option<&str>,
        cost: i32,
        is_enabled: bool,
        is_paused: bool,
        should_skip_request_queue: bool,
        is_managed: bool,
        prompt: Option<&str>,
        input_required: bool,
        command_name: Option<&str>,
    ) -> Result<CommandResult<CreateRedeemResult>, CommandError> {
        let mut metadata = std::collections::HashMap::new();
        if let Some(tid) = twitch_id {
            metadata.insert("twitch_id".to_string(), tid.to_string());
        }
        if let Some(pid) = plugin_id {
            metadata.insert("plugin_id".to_string(), pid.to_string());
        }
        if is_paused {
            metadata.insert("is_paused".to_string(), "true".to_string());
        }
        if should_skip_request_queue {
            metadata.insert("should_skip_request_queue".to_string(), "true".to_string());
        }
        if is_managed {
            metadata.insert("is_managed".to_string(), "true".to_string());
        }
        if let Some(p) = prompt {
            metadata.insert("prompt".to_string(), p.to_string());
        }
        if input_required {
            metadata.insert("input_required".to_string(), "true".to_string());
        }
        if let Some(cmd) = command_name {
            metadata.insert("command_name".to_string(), cmd.to_string());
        }
        
        let redeem = Redeem {
            redeem_id: String::new(), // Will be assigned by server
            platform: platform.to_string(),
            reward_id: String::new(), // Will be assigned if synced
            reward_name: redeem_name.to_string(),
            cost,
            is_active: is_enabled,
            is_dynamic: false,
            handler: String::new(),
            created_at: None,
            updated_at: None,
            metadata,
        };

        let request = CreateRedeemRequest {
            redeem: Some(redeem),
            sync_to_platform: true,
        };

        let response = client.redeem.clone()
            .create_redeem(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let redeem = response.into_inner().redeem
            .ok_or_else(|| CommandError::DataError("No redeem returned".to_string()))?;

        Ok(CommandResult {
            data: CreateRedeemResult { redeem },
            warnings: vec![],
        })
    }

    pub async fn get_redeem(
        client: &GrpcClient,
        redeem_id: &str,
    ) -> Result<CommandResult<GetRedeemResult>, CommandError> {
        let request = GetRedeemRequest {
            redeem_id: redeem_id.to_string(),
            include_usage: false,
        };

        let response = client.redeem.clone()
            .get_redeem(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let info = response.into_inner().redeem
            .ok_or_else(|| CommandError::DataError("No redeem returned".to_string()))?;
        
        let redeem = info.redeem
            .ok_or_else(|| CommandError::DataError("No redeem in info".to_string()))?;

        Ok(CommandResult {
            data: GetRedeemResult { redeem },
            warnings: vec![],
        })
    }

    pub async fn update_redeem(
        client: &GrpcClient,
        redeem_id: &str,
        redeem: Redeem,
    ) -> Result<CommandResult<UpdateRedeemResult>, CommandError> {
        let request = UpdateRedeemRequest {
            redeem_id: redeem_id.to_string(),
            redeem: Some(redeem),
            update_mask: None, // TODO: Add field mask support
            sync_to_platform: true,
        };

        let response = client.redeem.clone()
            .update_redeem(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let redeem = response.into_inner().redeem
            .ok_or_else(|| CommandError::DataError("No redeem returned".to_string()))?;

        Ok(CommandResult {
            data: UpdateRedeemResult { redeem },
            warnings: vec![],
        })
    }

    pub async fn delete_redeem(
        client: &GrpcClient,
        redeem_id: &str,
    ) -> Result<CommandResult<()>, CommandError> {
        let request = DeleteRedeemRequest {
            redeem_id: redeem_id.to_string(),
            remove_from_platform: true,
        };

        client.redeem.clone()
            .delete_redeem(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: (),
            warnings: vec![],
        })
    }

    pub async fn list_redeems(
        client: &GrpcClient,
        platform: Option<&str>,
        active_only: bool,
        page_size: i32,
    ) -> Result<CommandResult<ListRedeemsResult>, CommandError> {
        let request = ListRedeemsRequest {
            platform: platform.unwrap_or_default().to_string(),
            active_only,
            dynamic_only: false,
            page: Some(PageRequest {
                page_size,
                page_token: String::new(),
            }),
        };

        let response = client.redeem.clone()
            .list_redeems(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();
        let total_count = resp.page.as_ref()
            .map(|p| p.total_count)
            .unwrap_or(resp.redeems.len() as i32);

        Ok(CommandResult {
            data: ListRedeemsResult {
                redeems: resp.redeems,
                total_count,
            },
            warnings: vec![],
        })
    }

    pub async fn execute_redeem(
        client: &GrpcClient,
        redeem_id: &str,
        user_id: &str,
        platform_user_id: &str,
        input: Option<&str>,
    ) -> Result<CommandResult<ExecuteRedeemResult>, CommandError> {
        let request = ExecuteRedeemRequest {
            redeem_id: redeem_id.to_string(),
            user_id: user_id.to_string(),
            platform_user_id: platform_user_id.to_string(),
            user_input: input.unwrap_or_default().to_string(),
            context: Default::default(),
        };

        let response = client.redeem.clone()
            .execute_redeem(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();

        Ok(CommandResult {
            data: ExecuteRedeemResult {
                output: resp.response,
                success: resp.executed,
            },
            warnings: vec![],
        })
    }

    pub async fn sync_redeems(
        client: &GrpcClient,
        platform: &str,
    ) -> Result<CommandResult<SyncRedeemsResult>, CommandError> {
        let request = SyncRedeemsRequest {
            platforms: vec![platform.to_string()],
            direction: 3, // SYNC_DIRECTION_BIDIRECTIONAL
            dry_run: false,
        };

        let response = client.redeem.clone()
            .sync_redeems(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();

        Ok(CommandResult {
            data: SyncRedeemsResult {
                added_count: resp.created_count,
                updated_count: resp.updated_count,
                removed_count: resp.deleted_count,
            },
            warnings: vec![],
        })
    }

    pub async fn set_redeem_state(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
        is_enabled: Option<bool>,
        is_paused: Option<bool>,
    ) -> Result<CommandResult<UpdateRedeemResult>, CommandError> {
        if let Some(mut redeem) = Self::find_redeem_by_name(client, platform, redeem_name).await? {
            let redeem_id = redeem.redeem_id.clone();
            if let Some(enabled) = is_enabled {
                redeem.is_active = enabled;
            }
            if let Some(paused) = is_paused {
                redeem.metadata.insert("is_paused".to_string(), paused.to_string());
            }
            Self::update_redeem(client, &redeem_id, redeem).await
        } else {
            Err(CommandError::DataError(format!("Redeem '{}' not found on platform '{}'", redeem_name, platform)))
        }
    }

    pub async fn set_redeem_cost(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
        cost: i32,
    ) -> Result<CommandResult<UpdateRedeemResult>, CommandError> {
        if let Some(mut redeem) = Self::find_redeem_by_name(client, platform, redeem_name).await? {
            let redeem_id = redeem.redeem_id.clone();
            redeem.cost = cost;
            Self::update_redeem(client, &redeem_id, redeem).await
        } else {
            Err(CommandError::DataError(format!("Redeem '{}' not found on platform '{}'", redeem_name, platform)))
        }
    }

    pub async fn set_redeem_prompt(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
        prompt: &str,
    ) -> Result<CommandResult<UpdateRedeemResult>, CommandError> {
        if let Some(mut redeem) = Self::find_redeem_by_name(client, platform, redeem_name).await? {
            let redeem_id = redeem.redeem_id.clone();
            redeem.metadata.insert("prompt".to_string(), prompt.to_string());
            Self::update_redeem(client, &redeem_id, redeem).await
        } else {
            Err(CommandError::DataError(format!("Redeem '{}' not found on platform '{}'", redeem_name, platform)))
        }
    }

    // Helper method to find redeem by name
    pub async fn find_redeem_by_name(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
    ) -> Result<Option<Redeem>, CommandError> {
        let result = Self::list_redeems(client, Some(platform), false, 100).await?;
        
        Ok(result.data.redeems.into_iter()
            .filter_map(|info| info.redeem)
            .find(|r| r.reward_name.eq_ignore_ascii_case(redeem_name)))
    }

    // Helper to update specific fields
    pub async fn update_plugin(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
        plugin_id: &str,
    ) -> Result<CommandResult<UpdateRedeemResult>, CommandError> {
        if let Some(mut redeem) = Self::find_redeem_by_name(client, platform, redeem_name).await? {
            let redeem_id = redeem.redeem_id.clone();
            redeem.metadata.insert("plugin_id".to_string(), plugin_id.to_string());
            Self::update_redeem(client, &redeem_id, redeem).await
        } else {
            Err(CommandError::DataError(format!("Redeem '{}' not found on platform '{}'", redeem_name, platform)))
        }
    }

    pub async fn update_command(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
        command_name: &str,
    ) -> Result<CommandResult<UpdateRedeemResult>, CommandError> {
        if let Some(mut redeem) = Self::find_redeem_by_name(client, platform, redeem_name).await? {
            let redeem_id = redeem.redeem_id.clone();
            redeem.metadata.insert("command_name".to_string(), command_name.to_string());
            Self::update_redeem(client, &redeem_id, redeem).await
        } else {
            Err(CommandError::DataError(format!("Redeem '{}' not found on platform '{}'", redeem_name, platform)))
        }
    }

    pub async fn update_input_required(
        client: &GrpcClient,
        platform: &str,
        redeem_name: &str,
        input_required: bool,
    ) -> Result<CommandResult<UpdateRedeemResult>, CommandError> {
        if let Some(mut redeem) = Self::find_redeem_by_name(client, platform, redeem_name).await? {
            let redeem_id = redeem.redeem_id.clone();
            redeem.metadata.insert("input_required".to_string(), input_required.to_string());
            Self::update_redeem(client, &redeem_id, redeem).await
        } else {
            Err(CommandError::DataError(format!("Redeem '{}' not found on platform '{}'", redeem_name, platform)))
        }
    }
}