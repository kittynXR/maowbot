use tonic::{Request, Response, Status};
use maowbot_proto::maowbot::services::{config_service_server::ConfigService, *};
use maowbot_proto::maowbot::common::CacheControl;
use maowbot_core::repositories::postgres::bot_config::PostgresBotConfigRepository;
use maowbot_common::traits::repository_traits::BotConfigRepository;
use maowbot_core::eventbus::EventBus;
use std::sync::Arc;
use std::collections::HashMap;
use chrono::Utc;
use tracing::{info, error, debug, warn};
use prost_types;
use serde_json;

pub struct ConfigServiceImpl {
    bot_config_repo: Arc<PostgresBotConfigRepository>,
    event_bus: Arc<EventBus>,
}

impl ConfigServiceImpl {
    pub fn new(bot_config_repo: Arc<PostgresBotConfigRepository>, event_bus: Arc<EventBus>) -> Self {
        Self { bot_config_repo, event_bus }
    }
    
    fn value_to_config_type(value: &str) -> ConfigType {
        // Try to detect the type from the value
        if value == "true" || value == "false" {
            ConfigType::Boolean
        } else if value.parse::<i64>().is_ok() {
            ConfigType::Integer
        } else if value.parse::<f64>().is_ok() {
            ConfigType::Float
        } else if value.starts_with('{') || value.starts_with('[') {
            if serde_json::from_str::<serde_json::Value>(value).is_ok() {
                ConfigType::Json
            } else {
                ConfigType::String
            }
        } else {
            ConfigType::String
        }
    }
}

#[tonic::async_trait]
impl ConfigService for ConfigServiceImpl {
    async fn get_config(&self, request: Request<GetConfigRequest>) -> Result<Response<GetConfigResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting config for key: {}", req.key);
        
        // Get the value (metadata not supported in current API)
        let value = match self.bot_config_repo.get_value(&req.key).await {
            Ok(Some(v)) => v,
            Ok(None) => return Err(Status::not_found(format!("Config key '{}' not found", req.key))),
            Err(e) => return Err(Status::internal(format!("Failed to get config: {}", e))),
        };
        let meta: Option<serde_json::Value> = None; // TODO: Implement metadata support
        
        // Build metadata from JSON if available
        let metadata = if let Some(json_meta) = meta {
            let desc = json_meta.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let category = json_meta.get("category").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let is_secret = json_meta.get("is_secret").and_then(|v| v.as_bool()).unwrap_or(false);
            let is_required = json_meta.get("is_required").and_then(|v| v.as_bool()).unwrap_or(false);
            
            Some(ConfigMetadata {
                description: desc,
                r#type: Self::value_to_config_type(&value) as i32,
                default_value: String::new(),
                allowed_values: vec![],
                is_secret,
                is_required,
                category,
                created_at: None,
                updated_at: None,
                updated_by: String::new(),
            })
        } else {
            None
        };
        
        let config_entry = ConfigEntry {
            key: req.key.clone(),
            value,
            metadata,
        };
        
        Ok(Response::new(GetConfigResponse {
            config: Some(config_entry),
            cache: Some(CacheControl {
                ttl_seconds: 300,
                etag: String::new(),
                no_cache: false,
            }),
        }))
    }
    async fn set_config(&self, request: Request<SetConfigRequest>) -> Result<Response<SetConfigResponse>, Status> {
        let req = request.into_inner();
        info!("Setting config for key: {}", req.key);
        
        // Get the previous value if it exists
        let previous_value = self.bot_config_repo.get_value(&req.key).await
            .map_err(|e| Status::internal(format!("Failed to get previous value: {}", e)))?;
        
        let was_created = previous_value.is_none();
        
        if req.validate_only {
            // Just validate without saving
            return Ok(Response::new(SetConfigResponse {
                config: Some(ConfigEntry {
                    key: req.key,
                    value: req.value,
                    metadata: req.metadata.map(|m| ConfigMetadata {
                        description: m.description,
                        r#type: m.r#type,
                        default_value: m.default_value,
                        allowed_values: m.allowed_values,
                        is_secret: m.is_secret,
                        is_required: m.is_required,
                        category: m.category,
                        created_at: None,
                        updated_at: None,
                        updated_by: m.updated_by,
                    }),
                }),
                was_created,
                previous_value: previous_value.unwrap_or_default(),
            }));
        }
        
        // Build metadata JSON if provided
        let meta_json = if let Some(ref metadata) = req.metadata {
            let mut meta_map = serde_json::Map::new();
            if !metadata.description.is_empty() {
                meta_map.insert("description".to_string(), serde_json::Value::String(metadata.description.clone()));
            }
            if !metadata.category.is_empty() {
                meta_map.insert("category".to_string(), serde_json::Value::String(metadata.category.clone()));
            }
            meta_map.insert("is_secret".to_string(), serde_json::Value::Bool(metadata.is_secret));
            meta_map.insert("is_required".to_string(), serde_json::Value::Bool(metadata.is_required));
            meta_map.insert("type".to_string(), serde_json::Value::Number(metadata.r#type.into()));
            if !metadata.allowed_values.is_empty() {
                meta_map.insert("allowed_values".to_string(), 
                    serde_json::Value::Array(metadata.allowed_values.iter().map(|v| serde_json::Value::String(v.clone())).collect()));
            }
            Some(serde_json::Value::Object(meta_map))
        } else {
            None
        };
        
        // Save the config
        // If metadata is provided, use set_value_kv_meta, otherwise use simple set_value
        if meta_json.is_some() {
            self.bot_config_repo.set_value_kv_meta(&req.key, &req.value, meta_json).await
                .map_err(|e| Status::internal(format!("Failed to set config: {}", e)))?;
        } else {
            // For simple key-value pairs without metadata, use set_value
            self.bot_config_repo.set_value(&req.key, &req.value).await
                .map_err(|e| Status::internal(format!("Failed to set config: {}", e)))?;
        }
        
        // Build response
        let now = Utc::now();
        let metadata = req.metadata.map(|m| ConfigMetadata {
            description: m.description,
            r#type: m.r#type,
            default_value: m.default_value,
            allowed_values: m.allowed_values,
            is_secret: m.is_secret,
            is_required: m.is_required,
            category: m.category,
            created_at: if was_created {
                Some(prost_types::Timestamp {
                    seconds: now.timestamp(),
                    nanos: now.timestamp_subsec_nanos() as i32,
                })
            } else {
                None
            },
            updated_at: Some(prost_types::Timestamp {
                seconds: now.timestamp(),
                nanos: now.timestamp_subsec_nanos() as i32,
            }),
            updated_by: m.updated_by,
        });
        
        Ok(Response::new(SetConfigResponse {
            config: Some(ConfigEntry {
                key: req.key,
                value: req.value,
                metadata,
            }),
            was_created,
            previous_value: previous_value.unwrap_or_default(),
        }))
    }
    async fn delete_config(&self, request: Request<DeleteConfigRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        info!("Deleting config for key: {}", req.key);
        
        self.bot_config_repo.delete_value(&req.key).await
            .map_err(|e| Status::internal(format!("Failed to delete config: {}", e)))?;
        
        Ok(Response::new(()))
    }
    async fn list_configs(&self, request: Request<ListConfigsRequest>) -> Result<Response<ListConfigsResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing configs");
        
        // Get all configs
        let all_configs = self.bot_config_repo.list_all().await
            .map_err(|e| Status::internal(format!("Failed to list configs: {}", e)))?;
        
        let mut config_entries = Vec::new();
        
        for (key, value) in all_configs {
            // Filter by key prefix if specified
            if !req.key_prefix.is_empty() && !key.starts_with(&req.key_prefix) {
                continue;
            }
            
            // Parse metadata if available
            let metadata = if req.include_metadata {
                // TODO: Implement metadata support
                let json_meta: Option<serde_json::Value> = None;
                if let Some(json_meta) = json_meta {
                    let desc = json_meta.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let category = json_meta.get("category").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let is_secret = json_meta.get("is_secret").and_then(|v| v.as_bool()).unwrap_or(false);
                    let is_required = json_meta.get("is_required").and_then(|v| v.as_bool()).unwrap_or(false);
                    
                    // Filter by category if specified
                    if !req.categories.is_empty() && !req.categories.contains(&category) {
                        continue;
                    }
                    
                    // Skip secrets if not requested
                    if is_secret && !req.include_secrets {
                        continue;
                    }
                    
                    Some(ConfigMetadata {
                        description: desc,
                        r#type: Self::value_to_config_type(&value) as i32,
                        default_value: String::new(),
                        allowed_values: vec![],
                        is_secret,
                        is_required,
                        category,
                        created_at: None,
                        updated_at: None,
                        updated_by: String::new(),
                    })
                } else {
                    None
                }
            } else {
                None
            };
            
            config_entries.push(ConfigEntry {
                key,
                value: if metadata.as_ref().map(|m| m.is_secret).unwrap_or(false) && !req.include_secrets {
                    "<redacted>".to_string()
                } else {
                    value
                },
                metadata,
            });
        }
        
        // TODO: Implement proper pagination
        Ok(Response::new(ListConfigsResponse {
            configs: config_entries,
            page: None,
        }))
    }
    async fn batch_get_configs(&self, request: Request<BatchGetConfigsRequest>) -> Result<Response<BatchGetConfigsResponse>, Status> {
        let req = request.into_inner();
        debug!("Batch getting {} configs", req.keys.len());
        
        let mut configs = HashMap::new();
        let mut not_found_keys = Vec::new();
        
        for key in &req.keys {
            let result = self.bot_config_repo.get_value(key).await;
            
            match result {
                Ok(Some(value)) => {
                    let metadata = if req.include_metadata {
                        // TODO: Implement metadata support
                        let json_meta: Option<serde_json::Value> = None;
                        json_meta.and_then(|json_meta| {
                            let desc = json_meta.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let category = json_meta.get("category").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let is_secret = json_meta.get("is_secret").and_then(|v| v.as_bool()).unwrap_or(false);
                            let is_required = json_meta.get("is_required").and_then(|v| v.as_bool()).unwrap_or(false);
                            
                            Some(ConfigMetadata {
                                description: desc,
                                r#type: Self::value_to_config_type(&value) as i32,
                                default_value: String::new(),
                                allowed_values: vec![],
                                is_secret,
                                is_required,
                                category,
                                created_at: None,
                                updated_at: None,
                                updated_by: String::new(),
                            })
                        })
                    } else {
                        None
                    };
                    
                    configs.insert(key.clone(), ConfigEntry {
                        key: key.clone(),
                        value,
                        metadata,
                    });
                }
                Ok(None) => {
                    not_found_keys.push(key.clone());
                }
                Err(e) => {
                    error!("Error getting config {}: {}", key, e);
                    not_found_keys.push(key.clone());
                }
            }
        }
        
        Ok(Response::new(BatchGetConfigsResponse {
            configs,
            not_found_keys,
        }))
    }
    async fn batch_set_configs(&self, request: Request<BatchSetConfigsRequest>) -> Result<Response<BatchSetConfigsResponse>, Status> {
        let req = request.into_inner();
        info!("Batch setting {} configs", req.configs.len());
        
        let mut results = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;
        
        // If atomic is true and validate_all is true, validate all first
        if req.atomic && req.validate_all {
            // For now, we'll just check that all values are non-empty
            for (key, value) in &req.configs {
                if key.is_empty() || value.is_empty() {
                    return Err(Status::invalid_argument(format!("Invalid config: key='{}', value='{}'" , key, value)));
                }
            }
        }
        
        // Process each config
        for (key, value) in &req.configs {
            let result = self.bot_config_repo.set_value(key, value).await;
            
            match result {
                Ok(_) => {
                    success_count += 1;
                    results.push(SetResult {
                        key: key.clone(),
                        success: true,
                        error_message: String::new(),
                        config: Some(ConfigEntry {
                            key: key.clone(),
                            value: value.clone(),
                            metadata: None,
                        }),
                    });
                }
                Err(e) => {
                    failure_count += 1;
                    let error_msg = format!("Failed to set config: {}", e);
                    
                    if req.atomic {
                        // If atomic, rollback all successful operations
                        // Since we don't have transaction support, we'll just fail
                        return Err(Status::internal(format!("Atomic operation failed at key '{}': {}", key, e)));
                    }
                    
                    results.push(SetResult {
                        key: key.clone(),
                        success: false,
                        error_message: error_msg,
                        config: None,
                    });
                }
            }
        }
        
        Ok(Response::new(BatchSetConfigsResponse {
            results,
            success_count,
            failure_count,
        }))
    }
    async fn validate_config(&self, request: Request<ValidateConfigRequest>) -> Result<Response<ValidateConfigResponse>, Status> {
        let req = request.into_inner();
        debug!("Validating config - key: {}, value: {}", req.key, req.value);
        
        let mut errors = Vec::new();
        let mut is_valid = true;
        
        // Basic validation
        if req.key.is_empty() {
            is_valid = false;
            errors.push(ValidationError {
                field: "key".to_string(),
                message: "Config key cannot be empty".to_string(),
                severity: 2,
            });
        }
        
        if req.value.is_empty() {
            is_valid = false;
            errors.push(ValidationError {
                field: "value".to_string(),
                message: "Config value cannot be empty".to_string(),
                severity: 2,
            });
        }
        
        // Detect the actual type
        let detected_type = Self::value_to_config_type(&req.value);
        
        // If expected type is specified, validate against it
        if req.expected_type != ConfigType::Unknown as i32 {
            let expected = ConfigType::try_from(req.expected_type)
                .unwrap_or(ConfigType::Unknown);
            
            if expected != ConfigType::Unknown && expected != detected_type {
                is_valid = false;
                errors.push(ValidationError {
                    field: "value".to_string(),
                    message: format!("Expected type {:?} but got {:?}", expected, detected_type),
                    severity: 1,
                });
            }
        }
        
        // Additional type-specific validation
        match detected_type {
            ConfigType::Json => {
                if serde_json::from_str::<serde_json::Value>(&req.value).is_err() {
                    is_valid = false;
                    errors.push(ValidationError {
                        field: "value".to_string(),
                        message: "Invalid JSON format".to_string(),
                        severity: 2,
                    });
                }
            }
            _ => {}
        }
        
        Ok(Response::new(ValidateConfigResponse {
            is_valid,
            errors,
            detected_type: detected_type as i32,
        }))
    }
    async fn get_config_history(&self, request: Request<GetConfigHistoryRequest>) -> Result<Response<GetConfigHistoryResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting config history for key: {}", req.key);
        
        // Config history is not currently tracked in the database
        // Return empty history for now
        Ok(Response::new(GetConfigHistoryResponse {
            history: vec![],
        }))
    }
    async fn export_configs(&self, request: Request<ExportConfigsRequest>) -> Result<Response<ExportConfigsResponse>, Status> {
        let req = request.into_inner();
        info!("Exporting configs");
        
        // Get all configs
        let all_configs = self.bot_config_repo.list_all().await
            .map_err(|e| Status::internal(format!("Failed to list configs: {}", e)))?;
        
        let mut export_data = HashMap::new();
        let mut config_count = 0;
        
        for (key, value) in all_configs {
            // TODO: Implement category filtering when metadata is available
            
            // Skip secrets unless explicitly included (for now, skip anything with "secret" in the key)
            let is_secret = key.contains("secret") || key.contains("password") || key.contains("token");
            
            if is_secret && !req.include_secrets {
                continue;
            }
            
            // Build config object
            let mut config_obj = serde_json::Map::new();
            config_obj.insert("value".to_string(), serde_json::Value::String(value));
            
            // TODO: Add metadata when available
            // let meta_json: Option<serde_json::Value> = None;
            // if let Some(meta) = meta_json {
            //     config_obj.insert("metadata".to_string(), meta);
            // }
            
            export_data.insert(key, serde_json::Value::Object(config_obj));
            config_count += 1;
        }
        
        // Convert to requested format
        let export_result = match req.format() {
            ExportFormat::Json => {
                serde_json::to_string_pretty(&export_data)
                    .map_err(|e| Status::internal(format!("Failed to serialize to JSON: {}", e)))?
            }
            ExportFormat::Yaml => {
                // For YAML, we'll just use JSON for now
                serde_json::to_string_pretty(&export_data)
                    .map_err(|e| Status::internal(format!("Failed to serialize: {}", e)))?
            }
            _ => {
                return Err(Status::invalid_argument("Unsupported export format"));
            }
        };
        
        Ok(Response::new(ExportConfigsResponse {
            data: export_result.into_bytes(),
            filename: format!("maowbot_config_export_{}.json", Utc::now().format("%Y%m%d_%H%M%S")),
            config_count: config_count as i32,
        }))
    }
    async fn import_configs(&self, request: Request<ImportConfigsRequest>) -> Result<Response<ImportConfigsResponse>, Status> {
        let req = request.into_inner();
        let import_mode = req.mode();
        let import_format = req.format();
        info!("Importing configs");
        
        // Parse the import data based on format
        let import_data: HashMap<String, serde_json::Value> = match import_format {
            ExportFormat::Json => {
                let data_str = String::from_utf8(req.data)
                    .map_err(|e| Status::invalid_argument(format!("Invalid UTF-8 data: {}", e)))?;
                serde_json::from_str(&data_str)
                    .map_err(|e| Status::invalid_argument(format!("Invalid JSON: {}", e)))?
            }
            _ => {
                return Err(Status::invalid_argument("Unsupported import format"));
            }
        };
        
        let mut imported = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();
        
        for (key, config_data) in import_data {
            // Extract value and metadata
            let value = config_data.get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Status::invalid_argument(format!("Missing value for key: {}", key)))?;
            
            let meta_json = config_data.get("metadata").cloned();
            
            // Check if we should skip existing configs
            if import_mode == ImportMode::Merge {
                if let Ok(Some(_)) = self.bot_config_repo.get_value(&key).await {
                    skipped += 1;
                    continue;
                }
            }
            
            // Import the config (metadata not supported in set_value)
            match self.bot_config_repo.set_value(&key, value).await {
                Ok(_) => imported += 1,
                Err(e) => {
                    errors.push(format!("Failed to import {}: {}", key, e));
                }
            }
        }
        
        Ok(Response::new(ImportConfigsResponse {
            imported_count: imported,
            updated_count: 0, // TODO: Track updates separately
            skipped_count: skipped,
            errors: errors.into_iter().map(|e| ImportError {
                key: String::new(), // TODO: Extract key from error
                message: e,
                line_number: 0, // TODO: Track line numbers during import
            }).collect(),
        }))
    }
    type StreamConfigUpdatesStream = tonic::codec::Streaming<ConfigUpdateEvent>;
    async fn stream_config_updates(&self, _: Request<StreamConfigUpdatesRequest>) -> Result<Response<Self::StreamConfigUpdatesStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
    
    async fn shutdown_server(&self, request: Request<ShutdownServerRequest>) -> Result<Response<ShutdownServerResponse>, Status> {
        let req = request.into_inner();
        let reason = if req.reason.is_empty() { "User requested shutdown" } else { &req.reason };
        let grace_period = if req.grace_period_seconds > 0 { req.grace_period_seconds } else { 30 };
        
        info!("Server shutdown requested - reason: {}, grace period: {}s", reason, grace_period);
        
        // Get the current time + grace period
        let shutdown_time = Utc::now() + chrono::Duration::seconds(grace_period as i64);
        
        // Schedule shutdown after grace period
        let event_bus = self.event_bus.clone();
        tokio::spawn(async move {
            info!("Starting shutdown grace period of {} seconds", grace_period);
            tokio::time::sleep(tokio::time::Duration::from_secs(grace_period as u64)).await;
            info!("Grace period expired, triggering shutdown");
            event_bus.shutdown();
        });
        
        Ok(Response::new(ShutdownServerResponse {
            accepted: true,
            message: format!("Server shutdown scheduled in {} seconds - {}", grace_period, reason),
            shutdown_at: Some(prost_types::Timestamp {
                seconds: shutdown_time.timestamp(),
                nanos: shutdown_time.timestamp_subsec_nanos() as i32,
            }),
        }))
    }
}