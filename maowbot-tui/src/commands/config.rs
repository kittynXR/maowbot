//! config.rs – command handling for “config” subcommands:
//!
//!    config
//!         => show usage
//!
//!    config l | list
//!         => list all items from the bot_config table
//!
//!    config s | set <key> <value>
//!         => sets the given key to the provided value
//!
//!    config d | delete <key>
//!         => removes the given key from the bot_config table
//!
use std::sync::Arc;
use maowbot_common::traits::api::BotApi;

pub async fn handle_config_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return usage();
    }

    match args[0].to_lowercase().as_str() {
        "l" | "list" => {
            match bot_api.list_config().await {
                Ok(items) => {
                    if items.is_empty() {
                        "No config values found in bot_config table.".to_string()
                    } else {
                        let mut out = String::new();
                        for (k, v) in items {
                            out.push_str(&format!("{} = {}\n", k, v));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing config => {:?}", e),
            }
        }

        "s" | "set" => {
            if args.len() < 3 {
                return "Usage: config set <key> <value>".to_string();
            }
            let key = args[1];
            let value = args[2..].join(" "); // allow spaces in value
            match bot_api.set_bot_config_value(key, &value).await {
                Ok(_) => format!("Set '{}' to '{}'.", key, value),
                Err(e) => format!("Error setting config => {:?}", e),
            }
        }

        "d" | "delete" => {
            if args.len() < 2 {
                return "Usage: config delete <key>".to_string();
            }
            let key = args[1];
            match bot_api.delete_bot_config_key(key).await {
                Ok(_) => format!("Deleted config row for key='{}'.", key),
                Err(e) => format!("Error deleting config => {:?}", e),
            }
        }

        _ => usage(),
    }
}

fn usage() -> String {
    let mut out = String::new();
    out.push_str("Usage:\n");
    out.push_str("  config l|list             # list all items from bot_config table\n");
    out.push_str("  config s|set <key> <val>  # set key=value\n");
    out.push_str("  config d|delete <key>     # remove row by key\n");
    out
}
