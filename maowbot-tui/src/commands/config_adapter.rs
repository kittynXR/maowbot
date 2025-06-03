// Config command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::config::ConfigCommands};

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

        _ => usage(),
    }
}

fn usage() -> String {
    let mut out = String::new();
    out.push_str("Usage:\n");
    out.push_str("  config l|list             # list all items from bot_config table\n");
    out.push_str("  config g|get <key>        # get value for key\n");
    out.push_str("  config s|set <key> <val>  # set key=value\n");
    out.push_str("  config d|delete <key>     # remove row by key\n");
    out
}