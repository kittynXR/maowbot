use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{command_service_server::CommandService, *};
use maowbot_proto::maowbot::common;
use maowbot_common::traits::repository_traits::{CommandRepository, CommandUsageRepository};
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, error, debug};
use prost_types;

pub struct CommandServiceImpl {
    command_repo: Arc<dyn CommandRepository + Send + Sync>,
    command_usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
}

impl CommandServiceImpl {
    pub fn new(
        command_repo: Arc<dyn CommandRepository + Send + Sync>,
        command_usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
    ) -> Self {
        Self {
            command_repo,
            command_usage_repo,
        }
    }
    
    fn command_to_proto(cmd: &maowbot_common::models::command::Command) -> common::Command {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("min_role".to_string(), cmd.min_role.clone());
        metadata.insert("cooldown_warn_once".to_string(), cmd.cooldown_warnonce.to_string());
        metadata.insert("stream_online_only".to_string(), cmd.stream_online_only.to_string());
        metadata.insert("stream_offline_only".to_string(), cmd.stream_offline_only.to_string());
        if let Some(cred_id) = &cmd.respond_with_credential {
            metadata.insert("respond_with_credential".to_string(), cred_id.to_string());
        }
        if let Some(cred_id) = &cmd.active_credential_id {
            metadata.insert("active_credential_id".to_string(), cred_id.to_string());
        }
        
        common::Command {
            command_id: cmd.command_id.to_string(),
            platform: cmd.platform.clone(),
            name: cmd.command_name.clone(),
            description: String::new(), // Not in the Command struct
            is_active: cmd.is_active,
            cooldown_seconds: cmd.cooldown_seconds,
            required_roles: vec![], // TODO: Parse from min_role
            created_at: Some(prost_types::Timestamp {
                seconds: cmd.created_at.timestamp(),
                nanos: cmd.created_at.timestamp_subsec_nanos() as i32,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: cmd.updated_at.timestamp(),
                nanos: cmd.updated_at.timestamp_subsec_nanos() as i32,
            }),
            metadata,
        }
    }
    
    fn proto_to_command(proto: &common::Command) -> Result<maowbot_common::models::command::Command, Status> {
        let command_id = if proto.command_id.is_empty() {
            Uuid::new_v4()
        } else {
            Uuid::parse_str(&proto.command_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid command ID: {}", e)))?
        };
        
        // Extract fields from metadata
        let min_role = proto.metadata.get("min_role")
            .map(|s| s.clone())
            .unwrap_or_else(|| "Everyone".to_string());
            
        let cooldown_warnonce = proto.metadata.get("cooldown_warn_once")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
            
        let stream_online_only = proto.metadata.get("stream_online_only")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
            
        let stream_offline_only = proto.metadata.get("stream_offline_only")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
        
        let respond_with_credential = proto.metadata.get("respond_with_credential")
            .and_then(|id| Uuid::parse_str(id).ok());
            
        let active_credential_id = proto.metadata.get("active_credential_id")
            .and_then(|id| Uuid::parse_str(id).ok());
        
        Ok(maowbot_common::models::command::Command {
            command_id,
            platform: proto.platform.clone(),
            command_name: proto.name.clone(),
            min_role,
            is_active: proto.is_active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            cooldown_seconds: proto.cooldown_seconds,
            cooldown_warnonce,
            respond_with_credential,
            stream_online_only,
            stream_offline_only,
            active_credential_id,
        })
    }
}

#[tonic::async_trait]
impl CommandService for CommandServiceImpl {
    async fn list_commands(&self, request: Request<ListCommandsRequest>) -> Result<Response<ListCommandsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing commands - platform: {:?}, active_only: {}", req.platform, req.active_only);
        
        // Get commands for platform
        let commands = if req.platform.is_empty() {
            // If no platform specified, we'd need to list all - for now return empty
            vec![]
        } else {
            self.command_repo.list_commands(&req.platform).await
                .map_err(|e| Status::internal(format!("Failed to list commands: {}", e)))?
        };
        
        // Filter by active_only if requested
        let mut filtered_commands: Vec<_> = commands.into_iter()
            .filter(|cmd| !req.active_only || cmd.is_active)
            .filter(|cmd| req.name_prefix.is_empty() || cmd.command_name.starts_with(&req.name_prefix))
            .collect();
        
        // Sort by name
        filtered_commands.sort_by(|a, b| a.command_name.cmp(&b.command_name));
        
        // Convert to proto format
        let mut command_infos = Vec::new();
        for cmd in filtered_commands {
            // TODO: Get real statistics from usage data
            let stats = CommandStatistics {
                total_uses: 0,
                unique_users: 0,
                last_used: None,
                average_cooldown_wait: 0.0,
            };
            
            // Check if it's a builtin command by looking for certain patterns
            let is_builtin = cmd.command_name.starts_with('!') && 
                            (cmd.command_name == "!ping" || 
                             cmd.command_name == "!vanish" ||
                             cmd.command_name == "!followage");
            
            command_infos.push(CommandInfo {
                command: Some(Self::command_to_proto(&cmd)),
                stats: Some(stats),
                is_builtin,
            });
        }
        
        // TODO: Implement proper pagination
        Ok(Response::new(ListCommandsResponse {
            commands: command_infos,
            page: None,
        }))
    }
    async fn get_command(&self, request: Request<GetCommandRequest>) -> Result<Response<GetCommandResponse>, Status> {
        let req = request.into_inner();
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid command ID: {}", e)))?;
        
        debug!("Getting command: {}", command_id);
        
        let cmd = self.command_repo.get_command_by_id(command_id).await
            .map_err(|e| Status::internal(format!("Failed to get command: {}", e)))?;
        
        let cmd = match cmd {
            Some(c) => c,
            None => return Err(Status::not_found("Command not found")),
        };
        
        // Get usage data if requested
        let mut recent_usage = Vec::new();
        if req.include_usage {
            let usage_data = self.command_usage_repo.list_usage_for_command(command_id, 10).await
                .map_err(|e| Status::internal(format!("Failed to get usage data: {}", e)))?;
            
            for usage in usage_data {
                recent_usage.push(CommandUsageEntry {
                    user_id: usage.user_id.to_string(),
                    platform_user_id: String::new(), // TODO: Look up platform user ID
                    used_at: Some(prost_types::Timestamp {
                        seconds: usage.used_at.timestamp(),
                        nanos: usage.used_at.timestamp_subsec_nanos() as i32,
                    }),
                    channel: usage.channel.clone(),
                    was_on_cooldown: false, // TODO: Determine from metadata
                });
            }
        }
        
        // Build response
        let stats = CommandStatistics {
            total_uses: recent_usage.len() as i64,
            unique_users: 0, // TODO: Calculate unique users
            last_used: recent_usage.first().and_then(|u| u.used_at.clone()),
            average_cooldown_wait: 0.0,
        };
        
        let command_info = CommandInfo {
            command: Some(Self::command_to_proto(&cmd)),
            stats: Some(stats),
            is_builtin: false,
        };
        
        Ok(Response::new(GetCommandResponse {
            command: Some(command_info),
            recent_usage,
        }))
    }
    async fn create_command(&self, request: Request<CreateCommandRequest>) -> Result<Response<CreateCommandResponse>, Status> {
        let req = request.into_inner();
        let proto_cmd = req.command.ok_or_else(|| Status::invalid_argument("Command is required"))?;
        
        info!("Creating command: {} on platform {}", proto_cmd.name, proto_cmd.platform);
        
        // Validate command name
        if proto_cmd.name.is_empty() {
            return Err(Status::invalid_argument("Command name cannot be empty"));
        }
        
        // Check if command already exists
        let existing = self.command_repo.get_command_by_name(&proto_cmd.platform, &proto_cmd.name).await
            .map_err(|e| Status::internal(format!("Failed to check existing command: {}", e)))?;
        
        if existing.is_some() {
            return Err(Status::already_exists(format!("Command '{}' already exists on platform {}", 
                proto_cmd.name, proto_cmd.platform)));
        }
        
        if req.validate_only {
            // Just validate without creating
            return Ok(Response::new(CreateCommandResponse {
                command: Some(proto_cmd),
            }));
        }
        
        // Convert proto to model
        let mut cmd = Self::proto_to_command(&proto_cmd)?;
        cmd.created_at = Utc::now();
        cmd.updated_at = Utc::now();
        
        // Create the command
        self.command_repo.create_command(&cmd).await
            .map_err(|e| Status::internal(format!("Failed to create command: {}", e)))?;
        
        Ok(Response::new(CreateCommandResponse {
            command: Some(Self::command_to_proto(&cmd)),
        }))
    }
    async fn update_command(&self, request: Request<UpdateCommandRequest>) -> Result<Response<UpdateCommandResponse>, Status> {
        let req = request.into_inner();
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid command ID: {}", e)))?;
        
        info!("Updating command: {}", command_id);
        
        // Get existing command
        let existing = self.command_repo.get_command_by_id(command_id).await
            .map_err(|e| Status::internal(format!("Failed to get command: {}", e)))?;
        
        let mut existing = match existing {
            Some(c) => c,
            None => return Err(Status::not_found("Command not found")),
        };
        
        let proto_cmd = req.command.ok_or_else(|| Status::invalid_argument("Command is required"))?;
        
        // Apply updates based on field mask
        if let Some(mask) = req.update_mask {
            for path in &mask.paths {
                match path.as_str() {
                    "name" => existing.command_name = proto_cmd.name.clone(),
                    "min_role" => existing.min_role = proto_cmd.metadata.get("min_role")
                        .map(|s| s.clone())
                        .unwrap_or_else(|| existing.min_role.clone()),
                    "is_active" => existing.is_active = proto_cmd.is_active,
                    "cooldown_seconds" => existing.cooldown_seconds = proto_cmd.cooldown_seconds,
                    "cooldown_warn_once" => existing.cooldown_warnonce = proto_cmd.metadata.get("cooldown_warn_once")
                        .and_then(|s| s.parse::<bool>().ok())
                        .unwrap_or(existing.cooldown_warnonce),
                    "stream_online_only" => existing.stream_online_only = proto_cmd.metadata.get("stream_online_only")
                        .and_then(|s| s.parse::<bool>().ok())
                        .unwrap_or(existing.stream_online_only),
                    "stream_offline_only" => existing.stream_offline_only = proto_cmd.metadata.get("stream_offline_only")
                        .and_then(|s| s.parse::<bool>().ok())
                        .unwrap_or(existing.stream_offline_only),
                    "response_with" => {
                        existing.respond_with_credential = if let Some(id) = proto_cmd.metadata.get("respond_with_credential") {
                            Some(Uuid::parse_str(id)
                                .map_err(|e| Status::invalid_argument(format!("Invalid credential ID: {}", e)))?)
                        } else {
                            None
                        };
                    }
                    "active_credential_id" => {
                        existing.active_credential_id = if let Some(id) = proto_cmd.metadata.get("active_credential_id") {
                            Some(Uuid::parse_str(id)
                                .map_err(|e| Status::invalid_argument(format!("Invalid active credential ID: {}", e)))?)
                        } else {
                            None
                        };
                    }
                    _ => debug!("Unknown field in update mask: {}", path),
                }
            }
        } else {
            // No field mask - update all fields
            existing = Self::proto_to_command(&proto_cmd)?;
            existing.command_id = command_id; // Preserve the ID
        }
        
        existing.updated_at = Utc::now();
        
        // Update the command
        self.command_repo.update_command(&existing).await
            .map_err(|e| Status::internal(format!("Failed to update command: {}", e)))?;
        
        Ok(Response::new(UpdateCommandResponse {
            command: Some(Self::command_to_proto(&existing)),
        }))
    }
    async fn delete_command(&self, request: Request<DeleteCommandRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid command ID: {}", e)))?;
        
        info!("Deleting command: {}", command_id);
        
        if req.soft_delete {
            // Soft delete - just mark as inactive
            let existing = self.command_repo.get_command_by_id(command_id).await
                .map_err(|e| Status::internal(format!("Failed to get command: {}", e)))?;
            
            let mut existing = match existing {
                Some(c) => c,
                None => return Err(Status::not_found("Command not found")),
            };
            
            existing.is_active = false;
            existing.updated_at = Utc::now();
            
            self.command_repo.update_command(&existing).await
                .map_err(|e| Status::internal(format!("Failed to soft delete command: {}", e)))?;
        } else {
            // Hard delete
            self.command_repo.delete_command(command_id).await
                .map_err(|e| Status::internal(format!("Failed to delete command: {}", e)))?;
        }
        
        Ok(Response::new(()))
    }
    async fn batch_list_commands(&self, request: Request<BatchListCommandsRequest>) -> Result<Response<BatchListCommandsResponse>, Status> {
        let req = request.into_inner();
        debug!("Batch listing commands for {} platforms", req.platforms.len());
        
        let mut by_platform = HashMap::new();
        let mut all_commands = Vec::new();
        
        // Get commands for each platform
        for platform in &req.platforms {
            let commands = self.command_repo.list_commands(platform).await
                .map_err(|e| Status::internal(format!("Failed to list commands for {}: {}", platform, e)))?;
            
            let active_count = commands.iter().filter(|c| c.is_active).count() as i32;
            let total_count = commands.len() as i32;
            
            let mut command_infos = Vec::new();
            for cmd in commands {
                let stats = CommandStatistics {
                    total_uses: 0,
                    unique_users: 0,
                    last_used: None,
                    average_cooldown_wait: 0.0,
                };
                
                let info = CommandInfo {
                    command: Some(Self::command_to_proto(&cmd)),
                    stats: Some(stats),
                    is_builtin: false,
                };
                
                command_infos.push(info.clone());
                if !req.group_by_platform {
                    all_commands.push(info);
                }
            }
            
            if req.group_by_platform {
                by_platform.insert(platform.clone(), CommandList {
                    commands: command_infos,
                    active_count,
                    total_count,
                });
            }
        }
        
        Ok(Response::new(BatchListCommandsResponse {
            by_platform,
            all_commands,
        }))
    }
    async fn batch_update_commands(&self, request: Request<BatchUpdateCommandsRequest>) -> Result<Response<BatchUpdateCommandsResponse>, Status> {
        let req = request.into_inner();
        info!("Batch updating {} commands", req.updates.len());
        
        let mut results = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;
        
        for update in req.updates {
            let command_id = match Uuid::parse_str(&update.command_id) {
                Ok(id) => id,
                Err(e) => {
                    failure_count += 1;
                    results.push(UpdateResult {
                        command_id: update.command_id,
                        success: false,
                        command: None,
                        error_message: format!("Invalid command ID: {}", e),
                    });
                    if req.atomic {
                        return Err(Status::invalid_argument("Atomic operation failed due to invalid command ID"));
                    }
                    continue;
                }
            };
            
            // Get existing command
            let existing = match self.command_repo.get_command_by_id(command_id).await {
                Ok(Some(c)) => c,
                Ok(None) => {
                    failure_count += 1;
                    results.push(UpdateResult {
                        command_id: update.command_id,
                        success: false,
                        command: None,
                        error_message: "Command not found".to_string(),
                    });
                    if req.atomic {
                        return Err(Status::not_found("Atomic operation failed: command not found"));
                    }
                    continue;
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(UpdateResult {
                        command_id: update.command_id,
                        success: false,
                        command: None,
                        error_message: format!("Failed to get command: {}", e),
                    });
                    if req.atomic {
                        return Err(Status::internal("Atomic operation failed"));
                    }
                    continue;
                }
            };
            
            // Apply the update
            let mut updated = existing;
            if let Some(proto_cmd) = update.command {
                if let Some(mask) = update.update_mask {
                    // Apply field mask updates
                    for path in &mask.paths {
                        match path.as_str() {
                            "name" => updated.command_name = proto_cmd.name.clone(),
                            "min_role" => updated.min_role = proto_cmd.metadata.get("min_role")
                                .map(|s| s.clone())
                                .unwrap_or_else(|| updated.min_role.clone()),
                            "is_active" => updated.is_active = proto_cmd.is_active,
                            "cooldown_seconds" => updated.cooldown_seconds = proto_cmd.cooldown_seconds,
                            "cooldown_warn_once" => updated.cooldown_warnonce = proto_cmd.metadata.get("cooldown_warn_once")
                                .and_then(|s| s.parse::<bool>().ok())
                                .unwrap_or(updated.cooldown_warnonce),
                            _ => {}
                        }
                    }
                }
            }
            
            updated.updated_at = Utc::now();
            
            // Save the update
            match self.command_repo.update_command(&updated).await {
                Ok(_) => {
                    success_count += 1;
                    results.push(UpdateResult {
                        command_id: update.command_id,
                        success: true,
                        command: Some(Self::command_to_proto(&updated)),
                        error_message: String::new(),
                    });
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(UpdateResult {
                        command_id: update.command_id,
                        success: false,
                        command: None,
                        error_message: format!("Failed to update: {}", e),
                    });
                    if req.atomic {
                        return Err(Status::internal("Atomic operation failed during update"));
                    }
                }
            }
        }
        
        Ok(Response::new(BatchUpdateCommandsResponse {
            results,
            success_count,
            failure_count,
        }))
    }
    async fn execute_command(&self, request: Request<ExecuteCommandRequest>) -> Result<Response<ExecuteCommandResponse>, Status> {
        let req = request.into_inner();
        debug!("Executing command: {} on platform {} for user {}", req.command_name, req.platform, req.user_id);
        
        // Get the command
        let cmd = self.command_repo.get_command_by_name(&req.platform, &req.command_name).await
            .map_err(|e| Status::internal(format!("Failed to get command: {}", e)))?;
        
        let cmd = match cmd {
            Some(c) => c,
            None => {
                return Ok(Response::new(ExecuteCommandResponse {
                    executed: false,
                    response: String::new(),
                    cooldown: None,
                    error_message: "Command not found".to_string(),
                }));
            }
        };
        
        // Check if command is active
        if !cmd.is_active {
            return Ok(Response::new(ExecuteCommandResponse {
                executed: false,
                response: String::new(),
                cooldown: None,
                error_message: "Command is disabled".to_string(),
            }));
        }
        
        // TODO: Check cooldowns
        // TODO: Check permissions
        // TODO: Check stream online/offline requirements
        // TODO: Actually execute the command
        
        // For now, just return a mock response
        Ok(Response::new(ExecuteCommandResponse {
            executed: true,
            response: format!("Executed command '{}'", cmd.command_name),
            cooldown: if cmd.cooldown_seconds > 0 {
                Some(CooldownInfo {
                    on_cooldown: false,
                    remaining_seconds: 0,
                    available_at: Some(prost_types::Timestamp {
                        seconds: Utc::now().timestamp() + cmd.cooldown_seconds as i64,
                        nanos: 0,
                    }),
                })
            } else {
                None
            },
            error_message: String::new(),
        }))
    }
    async fn test_command(&self, request: Request<TestCommandRequest>) -> Result<Response<TestCommandResponse>, Status> {
        let req = request.into_inner();
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|e| Status::invalid_argument(format!("Invalid command ID: {}", e)))?;
        
        debug!("Testing command: {}", command_id);
        
        // Get the command
        let cmd = self.command_repo.get_command_by_id(command_id).await
            .map_err(|e| Status::internal(format!("Failed to get command: {}", e)))?;
        
        let cmd = match cmd {
            Some(c) => c,
            None => {
                return Ok(Response::new(TestCommandResponse {
                    would_execute: false,
                    expected_response: String::new(),
                    required_permissions: vec![],
                    error_message: "Command not found".to_string(),
                }));
            }
        };
        
        // Check if command would execute
        let would_execute = cmd.is_active;
        let required_permissions = vec![cmd.min_role.clone()];
        
        Ok(Response::new(TestCommandResponse {
            would_execute,
            expected_response: if would_execute {
                format!("Command '{}' would execute with input: {}", cmd.command_name, req.test_input)
            } else {
                String::new()
            },
            required_permissions,
            error_message: if !would_execute {
                "Command is disabled".to_string()
            } else {
                String::new()
            },
        }))
    }
    async fn get_command_usage(&self, request: Request<GetCommandUsageRequest>) -> Result<Response<GetCommandUsageResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting command usage");
        
        let mut usage_data = Vec::new();
        
        // If command_id is specified, get usage for that command
        if !req.command_id.is_empty() {
            let command_id = Uuid::parse_str(&req.command_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid command ID: {}", e)))?;
            
            // Get the command first
            let cmd = self.command_repo.get_command_by_id(command_id).await
                .map_err(|e| Status::internal(format!("Failed to get command: {}", e)))?;
            
            if let Some(cmd) = cmd {
                // Get usage for this command
                let usages = self.command_usage_repo.list_usage_for_command(command_id, 100).await
                    .map_err(|e| Status::internal(format!("Failed to get usage data: {}", e)))?;
                
                let use_count = usages.len() as i64;
                let unique_users = usages.iter()
                    .map(|u| u.user_id)
                    .collect::<std::collections::HashSet<_>>()
                    .len() as i64;
                
                let mut usage_by_channel = HashMap::new();
                for usage in &usages {
                    *usage_by_channel.entry(usage.channel.clone()).or_insert(0) += 1;
                }
                
                usage_data.push(CommandUsageData {
                    command_id: command_id.to_string(),
                    command_name: cmd.command_name,
                    timestamp: Some(prost_types::Timestamp {
                        seconds: Utc::now().timestamp(),
                        nanos: 0,
                    }),
                    use_count,
                    unique_users,
                    cooldown_hits: 0, // TODO: Track cooldown hits
                    usage_by_channel,
                });
            }
        } else if !req.platform.is_empty() {
            // Get all commands for platform and their usage
            let commands = self.command_repo.list_commands(&req.platform).await
                .map_err(|e| Status::internal(format!("Failed to list commands: {}", e)))?;
            
            for cmd in commands {
                // Get limited usage data for each command
                let usages = self.command_usage_repo.list_usage_for_command(cmd.command_id, 10).await
                    .map_err(|e| Status::internal(format!("Failed to get usage data: {}", e)))?;
                
                let use_count = usages.len() as i64;
                let unique_users = usages.iter()
                    .map(|u| u.user_id)
                    .collect::<std::collections::HashSet<_>>()
                    .len() as i64;
                
                usage_data.push(CommandUsageData {
                    command_id: cmd.command_id.to_string(),
                    command_name: cmd.command_name,
                    timestamp: Some(prost_types::Timestamp {
                        seconds: Utc::now().timestamp(),
                        nanos: 0,
                    }),
                    use_count,
                    unique_users,
                    cooldown_hits: 0,
                    usage_by_channel: HashMap::new(),
                });
            }
        }
        
        // Calculate summary
        let total_uses = usage_data.iter().map(|d| d.use_count).sum();
        let total_unique_users = usage_data.iter().map(|d| d.unique_users).sum();
        let most_used_command = usage_data.iter()
            .max_by_key(|d| d.use_count)
            .map(|d| d.command_name.clone())
            .unwrap_or_default();
        
        let summary = CommandUsageSummary {
            total_uses,
            total_unique_users,
            most_used_command,
            most_active_channel: String::new(), // TODO: Calculate across all channels
            average_uses_per_day: 0.0, // TODO: Calculate based on time range
        };
        
        Ok(Response::new(GetCommandUsageResponse {
            usage: usage_data,
            summary: Some(summary),
        }))
    }
    type StreamCommandEventsStream = tonic::codec::Streaming<CommandEvent>;
    async fn stream_command_events(&self, _: Request<StreamCommandEventsRequest>) -> Result<Response<Self::StreamCommandEventsStream>, Status> {
        // TODO: Implement streaming of command events
        Err(Status::unimplemented("Command event streaming not yet implemented"))
    }
}