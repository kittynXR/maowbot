// Config command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::config::ConfigCommands};
use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

pub async fn handle_config_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return usage();
    }

    match args[0].to_lowercase().as_str() {
        "l" | "list" => {
            match ConfigCommands::list_configs(client).await {
                Ok(result) => {
                    if result.configs.is_empty() {
                        "No config values found in bot_config table.".to_string()
                    } else {
                        let mut out = String::new();
                        for config in result.configs {
                            out.push_str(&format!("{} = {}\n", config.key, config.value));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing config => {}", e),
            }
        }

        "s" | "set" => {
            if args.len() < 3 {
                return "Usage: config set <key> <value>".to_string();
            }
            let key = args[1];
            let value = args[2..].join(" "); // allow spaces in value
            
            match ConfigCommands::set_config(client, key, &value).await {
                Ok(result) => {
                    if let Some(prev) = result.previous_value {
                        format!("Set '{}' to '{}' (was: '{}').", result.key, result.value, prev)
                    } else {
                        format!("Set '{}' to '{}'.", result.key, result.value)
                    }
                }
                Err(e) => format!("Error setting config => {}", e),
            }
        }

        "d" | "delete" => {
            if args.len() < 2 {
                return "Usage: config delete <key>".to_string();
            }
            let key = args[1];
            
            match ConfigCommands::delete_config(client, key).await {
                Ok(_) => format!("Deleted config row for key='{}'.", key),
                Err(e) => format!("Error deleting config => {}", e),
            }
        }

        "g" | "get" => {
            if args.len() < 2 {
                return "Usage: config get <key>".to_string();
            }
            let key = args[1];
            
            match ConfigCommands::get_config(client, key).await {
                Ok(result) => format!("{} = {}", result.key, result.value),
                Err(e) => format!("Error getting config => {}", e),
            }
        }
        
        "export" => {
            let filename = args.get(1).map(|s| *s).unwrap_or("bot_config_export.json");
            export_config(client, filename).await
        }
        
        "import" => {
            if args.len() < 2 {
                return "Usage: config import <filename> [--merge]".to_string();
            }
            let filename = args[1];
            let merge = args.get(2).map(|s| *s == "--merge").unwrap_or(false);
            import_config(client, filename, merge).await
        }

        _ => usage(),
    }
}

fn usage() -> String {
    let mut out = String::new();
    out.push_str("Usage:\n");
    out.push_str("  config l|list                  # list all items from bot_config table\n");
    out.push_str("  config g|get <key>             # get value for key\n");
    out.push_str("  config s|set <key> <val>       # set key=value\n");
    out.push_str("  config d|delete <key>          # remove row by key\n");
    out.push_str("  config export [filename]       # export all configs to JSON file\n");
    out.push_str("  config import <file> [--merge] # import configs from JSON (--merge to keep existing)\n");
    out
}

#[derive(Serialize, Deserialize)]
struct ConfigExport {
    version: String,
    exported_at: String,
    configs: HashMap<String, String>,
}

async fn export_config(client: &GrpcClient, filename: &str) -> String {
    // Get all configs
    match ConfigCommands::list_configs(client).await {
        Ok(result) => {
            let mut configs = HashMap::new();
            for config in result.configs {
                configs.insert(config.key, config.value);
            }
            
            let export_data = ConfigExport {
                version: "1.0".to_string(),
                exported_at: chrono::Utc::now().to_rfc3339(),
                configs,
            };
            
            match serde_json::to_string_pretty(&export_data) {
                Ok(json) => {
                    match fs::write(filename, json) {
                        Ok(_) => format!("Exported {} config entries to '{}'", export_data.configs.len(), filename),
                        Err(e) => format!("Error writing file '{}': {}", filename, e),
                    }
                }
                Err(e) => format!("Error serializing config: {}", e),
            }
        }
        Err(e) => format!("Error getting configs: {}", e),
    }
}

async fn import_config(client: &GrpcClient, filename: &str, merge: bool) -> String {
    // Check if file exists
    if !Path::new(filename).exists() {
        return format!("File '{}' not found", filename);
    }
    
    // Read file
    let contents = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(e) => return format!("Error reading file '{}': {}", filename, e),
    };
    
    // Parse JSON
    let export_data: ConfigExport = match serde_json::from_str(&contents) {
        Ok(data) => data,
        Err(e) => return format!("Error parsing JSON: {}", e),
    };
    
    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = 0;
    
    // If not merging, clear existing configs first
    if !merge {
        match ConfigCommands::list_configs(client).await {
            Ok(result) => {
                for config in result.configs {
                    match ConfigCommands::delete_config(client, &config.key).await {
                        Ok(_) => {},
                        Err(_) => errors += 1,
                    }
                }
            }
            Err(_) => return "Error clearing existing configs".to_string(),
        }
    }
    
    // Import each config
    for (key, value) in export_data.configs {
        if merge {
            // Check if key exists
            match ConfigCommands::get_config(client, &key).await {
                Ok(_) => {
                    skipped += 1;
                    continue;
                }
                Err(_) => {}, // Key doesn't exist, proceed with import
            }
        }
        
        match ConfigCommands::set_config(client, &key, &value).await {
            Ok(_) => imported += 1,
            Err(_) => errors += 1,
        }
    }
    
    let mut result = format!("Import complete: {} imported", imported);
    if skipped > 0 {
        result.push_str(&format!(", {} skipped (already exists)", skipped));
    }
    if errors > 0 {
        result.push_str(&format!(", {} errors", errors));
    }
    result
}