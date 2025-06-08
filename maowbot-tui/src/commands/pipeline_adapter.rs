// Pipeline command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::pipeline::PipelineCommands};
use std::io::{stdin, stdout, Write};

pub async fn handle_pipeline_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: pipeline <list|create|delete|toggle|show|filter|action|history|reload>".to_string();
    }

    match args[0] {
        "list" => {
            let include_disabled = args.get(1).map(|s| s == &"all").unwrap_or(false);
            
            match PipelineCommands::list_pipelines(client, include_disabled).await {
                Ok(result) => {
                    if result.data.pipelines.is_empty() {
                        "No pipelines found.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Event Pipelines:\n");
                        out.push_str("ID                                   | Name                | Priority | Enabled | Executions\n");
                        out.push_str("-------------------------------------|---------------------|----------|---------|------------\n");
                        
                        for pipeline in &result.data.pipelines {
                            out.push_str(&format!(
                                "{:36} | {:19} | {:8} | {:7} | {:>6} (S:{})\n",
                                truncate(&pipeline.pipeline_id, 36),
                                truncate(&pipeline.name, 19),
                                pipeline.priority,
                                if pipeline.enabled { "Yes" } else { "No" },
                                pipeline.execution_count,
                                pipeline.success_count,
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing pipelines: {}", e),
            }
        }
        
        "create" => {
            if args.len() < 2 {
                return "Usage: pipeline create <name> [description] [priority] [stop_on_match] [stop_on_error]".to_string();
            }
            
            let name = args[1];
            let description = args.get(2).unwrap_or(&"").to_string();
            let priority = args.get(3)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(100);
            let stop_on_match = args.get(4)
                .map(|s| s == &"true")
                .unwrap_or(false);
            let stop_on_error = args.get(5)
                .map(|s| s == &"true")
                .unwrap_or(false);
            
            match PipelineCommands::create_pipeline(
                client,
                name,
                &description,
                priority,
                stop_on_match,
                stop_on_error,
                vec![], // Empty tags for now
            ).await {
                Ok(result) => {
                    format!(
                        "Created pipeline '{}' (ID: {})",
                        result.data.pipeline.name,
                        result.data.pipeline.pipeline_id
                    )
                }
                Err(e) => format!("Error creating pipeline: {}", e),
            }
        }
        
        "delete" => {
            if args.len() < 2 {
                return "Usage: pipeline delete <id>".to_string();
            }
            
            let pipeline_id = args[1];
            
            // Confirm deletion
            println!("Are you sure you want to delete pipeline {}? (y/n): ", pipeline_id);
            print!("> ");
            let _ = stdout().flush();
            
            let mut line = String::new();
            let _ = stdin().read_line(&mut line);
            
            if line.trim().to_lowercase() != "y" {
                return "Pipeline deletion cancelled.".to_string();
            }
            
            match PipelineCommands::delete_pipeline(client, pipeline_id).await {
                Ok(_) => format!("Pipeline {} deleted successfully.", pipeline_id),
                Err(e) => format!("Error deleting pipeline: {}", e),
            }
        }
        
        "toggle" => {
            if args.len() < 3 {
                return "Usage: pipeline toggle <id> <enabled|disabled>".to_string();
            }
            
            let pipeline_id = args[1];
            let enabled = match args[2] {
                "enabled" | "enable" | "on" => true,
                "disabled" | "disable" | "off" => false,
                _ => return "Invalid toggle state. Use 'enabled' or 'disabled'.".to_string(),
            };
            
            match PipelineCommands::toggle_pipeline(client, pipeline_id, enabled).await {
                Ok(_) => format!(
                    "Pipeline {} {}.",
                    pipeline_id,
                    if enabled { "enabled" } else { "disabled" }
                ),
                Err(e) => format!("Error toggling pipeline: {}", e),
            }
        }
        
        "show" => {
            if args.len() < 2 {
                return "Usage: pipeline show <id>".to_string();
            }
            
            let pipeline_id = args[1];
            match PipelineCommands::get_pipeline(client, pipeline_id).await {
                Ok(result) => {
                    let pipeline = &result.data.pipeline;
                    let mut out = String::new();
                    
                    out.push_str(&format!("Pipeline Details:\n"));
                    out.push_str(&format!("  ID: {}\n", pipeline.pipeline_id));
                    out.push_str(&format!("  Name: {}\n", pipeline.name));
                    out.push_str(&format!("  Description: {}\n", pipeline.description));
                    out.push_str(&format!("  Priority: {}\n", pipeline.priority));
                    out.push_str(&format!("  Enabled: {}\n", if pipeline.enabled { "Yes" } else { "No" }));
                    out.push_str(&format!("  Stop on Match: {}\n", if pipeline.stop_on_match { "Yes" } else { "No" }));
                    out.push_str(&format!("  Stop on Error: {}\n", if pipeline.stop_on_error { "Yes" } else { "No" }));
                    out.push_str(&format!("  System Pipeline: {}\n", if pipeline.is_system { "Yes" } else { "No" }));
                    out.push_str(&format!("  Tags: {}\n", pipeline.tags.join(", ")));
                    out.push_str(&format!("  Execution Stats: {} total ({} success)\n",
                        pipeline.execution_count,
                        pipeline.success_count
                    ));
                    if !pipeline.last_executed.is_empty() {
                        out.push_str(&format!("  Last Executed: {}\n", pipeline.last_executed));
                    }
                    out.push_str(&format!("  Created: {}\n", pipeline.created_at));
                    out.push_str(&format!("  Updated: {}\n", pipeline.updated_at));
                    
                    // Get filters and actions
                    if let Ok(filters_result) = PipelineCommands::list_filters(client, pipeline_id).await {
                        out.push_str("\nFilters:\n");
                        if filters_result.data.filters.is_empty() {
                            out.push_str("  (none)\n");
                        } else {
                            for filter in &filters_result.data.filters {
                                out.push_str(&format!(
                                    "  [{}] {} - Order: {} (Negated: {}, Required: {})\n",
                                    filter.filter_id,
                                    filter.filter_type,
                                    filter.filter_order,
                                    if filter.is_negated { "Yes" } else { "No" },
                                    if filter.is_required { "Yes" } else { "No" }
                                ));
                            }
                        }
                    }
                    
                    if let Ok(actions_result) = PipelineCommands::list_actions(client, pipeline_id).await {
                        out.push_str("\nActions:\n");
                        if actions_result.data.actions.is_empty() {
                            out.push_str("  (none)\n");
                        } else {
                            for action in &actions_result.data.actions {
                                out.push_str(&format!(
                                    "  [{}] {} - Order: {} (Continue on error: {})\n",
                                    action.action_id,
                                    action.action_type,
                                    action.action_order,
                                    if action.continue_on_error { "Yes" } else { "No" }
                                ));
                            }
                        }
                    }
                    
                    out
                }
                Err(e) => format!("Error getting pipeline details: {}", e),
            }
        }
        
        "filter" => {
            if args.len() < 2 {
                return "Usage: pipeline filter <add|remove|list|types>".to_string();
            }
            
            match args[1] {
                "add" => {
                    if args.len() < 4 {
                        return "Usage: pipeline filter add <pipeline_id> <filter_type> [config_json] [order_index] [negated] [required]".to_string();
                    }
                    
                    let pipeline_id = args[2];
                    let filter_type = args[3];
                    let filter_config = args.get(4).unwrap_or(&"{}");
                    let filter_order = args.get(5)
                        .and_then(|s| s.parse::<i32>().ok());
                    let is_negated = args.get(6)
                        .map(|s| s == &"true")
                        .unwrap_or(false);
                    let is_required = args.get(7)
                        .map(|s| s == &"true")
                        .unwrap_or(false);
                    
                    match PipelineCommands::add_filter(
                        client,
                        pipeline_id,
                        filter_type,
                        filter_config,
                        filter_order,
                        is_negated,
                        is_required,
                    ).await {
                        Ok(result) => {
                            format!(
                                "Added filter '{}' (ID: {}) to pipeline {}",
                                result.data.filter.filter_type,
                                result.data.filter.filter_id,
                                pipeline_id
                            )
                        }
                        Err(e) => format!("Error adding filter: {}", e),
                    }
                }
                
                "remove" => {
                    if args.len() < 3 {
                        return "Usage: pipeline filter remove <filter_id>".to_string();
                    }
                    
                    let filter_id = args[2];
                    match PipelineCommands::remove_filter(client, filter_id).await {
                        Ok(_) => format!("Filter {} removed successfully.", filter_id),
                        Err(e) => format!("Error removing filter: {}", e),
                    }
                }
                
                "list" => {
                    if args.len() < 3 {
                        return "Usage: pipeline filter list <pipeline_id>".to_string();
                    }
                    
                    let pipeline_id = args[2];
                    match PipelineCommands::list_filters(client, pipeline_id).await {
                        Ok(result) => {
                            if result.data.filters.is_empty() {
                                format!("No filters found for pipeline {}.", pipeline_id)
                            } else {
                                let mut out = String::new();
                                out.push_str(&format!("Filters for pipeline {}:\n", pipeline_id));
                                for filter in &result.data.filters {
                                    out.push_str(&format!(
                                        "  [{}] {} - Order: {} - Negated: {} - Required: {} - Config: {}\n",
                                        filter.filter_id,
                                        filter.filter_type,
                                        filter.filter_order,
                                        if filter.is_negated { "Yes" } else { "No" },
                                        if filter.is_required { "Yes" } else { "No" },
                                        truncate(&filter.filter_config, 40)
                                    ));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing filters: {}", e),
                    }
                }
                
                "types" => {
                    match PipelineCommands::get_available_filters(client).await {
                        Ok(result) => {
                            let mut out = String::new();
                            out.push_str("Available Filter Types:\n");
                            for filter in &result.data.filters {
                                out.push_str(&format!(
                                    "  {} - {}\n",
                                    filter.id,
                                    filter.description
                                ));
                            }
                            out
                        }
                        Err(e) => format!("Error getting filter types: {}", e),
                    }
                }
                
                _ => "Usage: pipeline filter <add|remove|list|types>".to_string(),
            }
        }
        
        "action" => {
            if args.len() < 2 {
                return "Usage: pipeline action <add|remove|list|types>".to_string();
            }
            
            match args[1] {
                "add" => {
                    if args.len() < 4 {
                        return "Usage: pipeline action add <pipeline_id> <action_type> [config_json] [order_index] [continue_on_error] [is_async] [timeout_ms] [retry_count] [retry_delay_ms]".to_string();
                    }
                    
                    let pipeline_id = args[2];
                    let action_type = args[3];
                    let action_config = args.get(4).unwrap_or(&"{}");
                    let action_order = args.get(5)
                        .and_then(|s| s.parse::<i32>().ok());
                    let continue_on_error = args.get(6)
                        .map(|s| s == &"true")
                        .unwrap_or(false);
                    let is_async = args.get(7)
                        .map(|s| s == &"true")
                        .unwrap_or(false);
                    let timeout_ms = args.get(8)
                        .and_then(|s| s.parse::<i32>().ok());
                    let retry_count = args.get(9)
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    let retry_delay_ms = args.get(10)
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(1000);
                    
                    match PipelineCommands::add_action(
                        client,
                        pipeline_id,
                        action_type,
                        action_config,
                        action_order,
                        continue_on_error,
                        is_async,
                        timeout_ms,
                        retry_count,
                        retry_delay_ms,
                    ).await {
                        Ok(result) => {
                            format!(
                                "Added action '{}' (ID: {}) to pipeline {}",
                                result.data.action.action_type,
                                result.data.action.action_id,
                                pipeline_id
                            )
                        }
                        Err(e) => format!("Error adding action: {}", e),
                    }
                }
                
                "remove" => {
                    if args.len() < 3 {
                        return "Usage: pipeline action remove <action_id>".to_string();
                    }
                    
                    let action_id = args[2];
                    match PipelineCommands::remove_action(client, action_id).await {
                        Ok(_) => format!("Action {} removed successfully.", action_id),
                        Err(e) => format!("Error removing action: {}", e),
                    }
                }
                
                "list" => {
                    if args.len() < 3 {
                        return "Usage: pipeline action list <pipeline_id>".to_string();
                    }
                    
                    let pipeline_id = args[2];
                    match PipelineCommands::list_actions(client, pipeline_id).await {
                        Ok(result) => {
                            if result.data.actions.is_empty() {
                                format!("No actions found for pipeline {}.", pipeline_id)
                            } else {
                                let mut out = String::new();
                                out.push_str(&format!("Actions for pipeline {}:\n", pipeline_id));
                                for action in &result.data.actions {
                                    out.push_str(&format!(
                                        "  [{}] {} - Order: {} - Continue on error: {} - Async: {} - Config: {}\n",
                                        action.action_id,
                                        action.action_type,
                                        action.action_order,
                                        if action.continue_on_error { "Yes" } else { "No" },
                                        if action.is_async { "Yes" } else { "No" },
                                        truncate(&action.action_config, 30)
                                    ));
                                }
                                out
                            }
                        }
                        Err(e) => format!("Error listing actions: {}", e),
                    }
                }
                
                "types" => {
                    match PipelineCommands::get_available_actions(client).await {
                        Ok(result) => {
                            let mut out = String::new();
                            out.push_str("Available Action Types:\n");
                            for action in &result.data.actions {
                                out.push_str(&format!(
                                    "  {} - {} (Parallelizable: {})\n",
                                    action.id,
                                    action.description,
                                    if action.is_parallelizable { "Yes" } else { "No" }
                                ));
                            }
                            out
                        }
                        Err(e) => format!("Error getting action types: {}", e),
                    }
                }
                
                _ => "Usage: pipeline action <add|remove|list|types>".to_string(),
            }
        }
        
        "history" => {
            let pipeline_id = args.get(1).map(|s| s.to_string());
            let limit = args.get(2)
                .and_then(|s| s.parse::<i32>().ok())
                .or(Some(20));
            let offset = args.get(3)
                .and_then(|s| s.parse::<i32>().ok());
            
            match PipelineCommands::get_execution_history(client, pipeline_id.as_deref(), limit, offset).await {
                Ok(result) => {
                    if result.data.executions.is_empty() {
                        "No execution history found.".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str(&format!(
                            "Execution History (showing {} of {}):\n",
                            result.data.executions.len(),
                            result.data.total_count
                        ));
                        out.push_str("Execution ID                         | Pipeline          | Event Type      | Status  | Started\n");
                        out.push_str("-------------------------------------|-------------------|-----------------|---------|--------------------\n");
                        
                        for exec in &result.data.executions {
                            out.push_str(&format!(
                                "{} | {:17} | {:15} | {:7} | {}\n",
                                exec.execution_id,
                                truncate(&exec.pipeline_name, 17),
                                truncate(&exec.event_type, 15),
                                truncate(&exec.status, 7),
                                truncate(&exec.started_at, 20)
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error getting execution history: {}", e),
            }
        }
        
        "reload" => {
            match PipelineCommands::reload_pipelines(client).await {
                Ok(result) => {
                    format!("Pipelines reloaded successfully. {} pipelines loaded.", result.data.pipelines_loaded)
                }
                Err(e) => format!("Error reloading pipelines: {}", e),
            }
        }
        
        _ => "Usage: pipeline <list|create|delete|toggle|show|filter|action|history|reload>".to_string(),
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len-3])
    }
}