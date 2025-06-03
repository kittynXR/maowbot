use crate::{GrpcClient, CommandResult, CommandError};
use maowbot_proto::maowbot::services::{
    CreateCommandRequest, GetCommandRequest, UpdateCommandRequest,
    DeleteCommandRequest, ListCommandsRequest, ExecuteCommandRequest,
    GetCommandUsageRequest, CommandInfo,
};
use maowbot_proto::maowbot::common::{Command, PageRequest};
use uuid::Uuid;

// Result structures
pub struct CreateCommandResult {
    pub command: Command,
}

pub struct GetCommandResult {
    pub command: Command,
}

pub struct UpdateCommandResult {
    pub command: Command,
}

pub struct ListCommandsResult {
    pub commands: Vec<CommandInfo>,
    pub total_count: i32,
}

pub struct ExecuteCommandResult {
    pub output: String,
    pub success: bool,
}

pub struct GetCommandUsageResult {
    pub total_uses: i64,
    pub unique_users: i64,
}

// Command handlers
pub struct CommandCommands;

impl CommandCommands {
    pub async fn create_command(
        client: &GrpcClient,
        platform: &str,
        command_name: &str,
        plugin_id: Option<&str>,
        is_active: bool,
        cooldown_seconds: i32,
        cooldown_warnonce: bool,
        respond_with_credential: Option<&str>,
    ) -> Result<CommandResult<CreateCommandResult>, CommandError> {
        let mut metadata = std::collections::HashMap::new();
        if let Some(pid) = plugin_id {
            metadata.insert("plugin_id".to_string(), pid.to_string());
        }
        if cooldown_warnonce {
            metadata.insert("cooldown_warnonce".to_string(), "true".to_string());
        }
        if let Some(cred) = respond_with_credential {
            metadata.insert("respond_with_credential".to_string(), cred.to_string());
        }
        
        let command = Command {
            command_id: String::new(), // Will be assigned by server
            platform: platform.to_string(),
            name: command_name.to_string(),
            description: String::new(),
            is_active,
            cooldown_seconds,
            required_roles: vec![],
            created_at: None,
            updated_at: None,
            metadata,
        };

        let request = CreateCommandRequest {
            command: Some(command),
            validate_only: false,
        };

        let response = client.command.clone()
            .create_command(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let command = response.into_inner().command
            .ok_or_else(|| CommandError::DataError("No command returned".to_string()))?;

        Ok(CommandResult {
            data: CreateCommandResult { command },
            warnings: vec![],
        })
    }

    pub async fn get_command(
        client: &GrpcClient,
        command_id: &str,
    ) -> Result<CommandResult<GetCommandResult>, CommandError> {
        let request = GetCommandRequest {
            command_id: command_id.to_string(),
            include_usage: false,
        };

        let response = client.command.clone()
            .get_command(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let info = response.into_inner().command
            .ok_or_else(|| CommandError::DataError("No command returned".to_string()))?;
        
        let command = info.command
            .ok_or_else(|| CommandError::DataError("No command in info".to_string()))?;

        Ok(CommandResult {
            data: GetCommandResult { command },
            warnings: vec![],
        })
    }

    pub async fn update_command(
        client: &GrpcClient,
        command_id: &str,
        command: Command,
    ) -> Result<CommandResult<UpdateCommandResult>, CommandError> {
        let request = UpdateCommandRequest {
            command_id: command_id.to_string(),
            command: Some(command),
            update_mask: None, // TODO: Add field mask support
        };

        let response = client.command.clone()
            .update_command(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let command = response.into_inner().command
            .ok_or_else(|| CommandError::DataError("No command returned".to_string()))?;

        Ok(CommandResult {
            data: UpdateCommandResult { command },
            warnings: vec![],
        })
    }

    pub async fn delete_command(
        client: &GrpcClient,
        command_id: &str,
    ) -> Result<CommandResult<()>, CommandError> {
        let request = DeleteCommandRequest {
            command_id: command_id.to_string(),
            soft_delete: false,
        };

        client.command.clone()
            .delete_command(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        Ok(CommandResult {
            data: (),
            warnings: vec![],
        })
    }

    pub async fn list_commands(
        client: &GrpcClient,
        platform: Option<&str>,
        active_only: bool,
        page_size: i32,
    ) -> Result<CommandResult<ListCommandsResult>, CommandError> {
        let request = ListCommandsRequest {
            platform: platform.unwrap_or_default().to_string(),
            active_only,
            name_prefix: String::new(),
            page: Some(PageRequest {
                page_size,
                page_token: String::new(),
            }),
        };

        let response = client.command.clone()
            .list_commands(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();
        let total_count = resp.page.as_ref()
            .map(|p| p.total_count)
            .unwrap_or(resp.commands.len() as i32);

        Ok(CommandResult {
            data: ListCommandsResult {
                commands: resp.commands,
                total_count,
            },
            warnings: vec![],
        })
    }

    pub async fn execute_command(
        client: &GrpcClient,
        platform: &str,
        command_name: &str,
        user_id: &str,
        channel: &str,
        args: Vec<String>,
    ) -> Result<CommandResult<ExecuteCommandResult>, CommandError> {
        let request = ExecuteCommandRequest {
            platform: platform.to_string(),
            command_name: command_name.to_string(),
            user_id: user_id.to_string(),
            channel: channel.to_string(),
            arguments: args,
            context: Default::default(),
        };

        let response = client.command.clone()
            .execute_command(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();

        Ok(CommandResult {
            data: ExecuteCommandResult {
                output: resp.response,
                success: resp.executed,
            },
            warnings: vec![],
        })
    }

    pub async fn get_command_usage(
        client: &GrpcClient,
        command_id: &str,
        platform: &str,
    ) -> Result<CommandResult<GetCommandUsageResult>, CommandError> {
        let request = GetCommandUsageRequest {
            command_id: command_id.to_string(),
            platform: platform.to_string(),
            start_time: None,
            end_time: None,
            grouping: 0, // COMMAND_USAGE_GROUPING_NONE
        };

        let response = client.command.clone()
            .get_command_usage(request)
            .await
            .map_err(|e| CommandError::GrpcError(e.to_string()))?;

        let resp = response.into_inner();
        let summary = resp.summary
            .ok_or_else(|| CommandError::DataError("No usage summary returned".to_string()))?;

        Ok(CommandResult {
            data: GetCommandUsageResult { 
                total_uses: summary.total_uses,
                unique_users: summary.total_unique_users,
            },
            warnings: vec![],
        })
    }


    // Helper method to find command by name and platform
    pub async fn find_command_by_name(
        client: &GrpcClient,
        platform: &str,
        command_name: &str,
    ) -> Result<Option<Command>, CommandError> {
        let result = Self::list_commands(client, Some(platform), false, 100).await?;
        
        Ok(result.data.commands.into_iter()
            .filter_map(|c| c.command)
            .find(|c| c.name.eq_ignore_ascii_case(command_name)))
    }

    // Helper to update specific fields
    pub async fn update_cooldown(
        client: &GrpcClient,
        platform: &str,
        command_name: &str,
        cooldown_seconds: i32,
    ) -> Result<CommandResult<UpdateCommandResult>, CommandError> {
        if let Some(mut cmd) = Self::find_command_by_name(client, platform, command_name).await? {
            let command_id = cmd.command_id.clone();
            cmd.cooldown_seconds = cooldown_seconds;
            Self::update_command(client, &command_id, cmd).await
        } else {
            Err(CommandError::DataError(format!("Command '{}' not found on platform '{}'", command_name, platform)))
        }
    }

    pub async fn update_warnonce(
        client: &GrpcClient,
        platform: &str,
        command_name: &str,
        cooldown_warnonce: bool,
    ) -> Result<CommandResult<UpdateCommandResult>, CommandError> {
        if let Some(mut cmd) = Self::find_command_by_name(client, platform, command_name).await? {
            let command_id = cmd.command_id.clone();
            cmd.metadata.insert("cooldown_warnonce".to_string(), cooldown_warnonce.to_string());
            Self::update_command(client, &command_id, cmd).await
        } else {
            Err(CommandError::DataError(format!("Command '{}' not found on platform '{}'", command_name, platform)))
        }
    }

    pub async fn update_respond_with(
        client: &GrpcClient,
        platform: &str,
        command_name: &str,
        credential_id: Option<String>,
    ) -> Result<CommandResult<UpdateCommandResult>, CommandError> {
        if let Some(mut cmd) = Self::find_command_by_name(client, platform, command_name).await? {
            let command_id = cmd.command_id.clone();
            if let Some(cred_id) = credential_id {
                cmd.metadata.insert("respond_with_credential".to_string(), cred_id);
            } else {
                cmd.metadata.remove("respond_with_credential");
            }
            Self::update_command(client, &command_id, cmd).await
        } else {
            Err(CommandError::DataError(format!("Command '{}' not found on platform '{}'", command_name, platform)))
        }
    }

    pub async fn set_active(
        client: &GrpcClient,
        platform: &str,
        command_name: &str,
        is_active: bool,
    ) -> Result<CommandResult<UpdateCommandResult>, CommandError> {
        if let Some(mut cmd) = Self::find_command_by_name(client, platform, command_name).await? {
            let command_id = cmd.command_id.clone();
            cmd.is_active = is_active;
            Self::update_command(client, &command_id, cmd).await
        } else {
            Err(CommandError::DataError(format!("Command '{}' not found on platform '{}'", command_name, platform)))
        }
    }
}