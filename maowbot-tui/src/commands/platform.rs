// =============================================================================
// maowbot-tui/src/commands/platform.rs

use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use std::str::FromStr;

/// platform add <platform>
/// platform remove <platform_config_id>  (or some approach)
/// platform list [optional: <platform_name>]
pub fn handle_platform_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: platform <add|remove|list> ...".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: platform add <platformName>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(plat) => handle_platform_add(plat, bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "remove" => {
            // For simplicity, we might remove by an ID or label. The code below just prompts:
            handle_platform_remove(bot_api)
        }
        "list" => {
            let maybe_platform = if args.len() > 1 {
                Some(args[1])
            } else {
                None
            };
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            // reusing "list_credentials" isn't correct, we want to list PLATFORM configs:
            let list_res = rt.block_on(bot_api.list_platform_configs(maybe_platform));
            match list_res {
                Ok(list) => {
                    if list.is_empty() {
                        "No platform configs found.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Current platform configs:\n");
                        for pc in list {
                            out.push_str(&format!(
                                " - id={} platform={} label={} client_id={}\n",
                                pc.platform_config_id,
                                pc.platform,
                                pc.platform_label.as_deref().unwrap_or(""),
                                pc.client_id.as_deref().unwrap_or("NONE"),
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing platform configs => {:?}", e),
            }
        }
        _ => "Usage: platform <add|remove|list>".to_string(),
    }
}

/// Interactively ask for label, client_id, client_secret, then store in DB.
fn handle_platform_add(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    println!("You are adding a new platform config for '{}'.", platform_str);
    println!("Enter a label (e.g. 'bot1' or 'user1'): ");
    print!("> ");
    let _ = stdout().flush();

    let mut label_line = String::new();
    let _ = stdin().read_line(&mut label_line);
    let label = label_line.trim().to_string();
    if label.is_empty() {
        return "No label provided; aborted.".to_string();
    }

    // Some platforms might not require client_id/secret, but we'll just ask anyway:
    println!("Enter client_id (or leave blank if not needed): ");
    print!("> ");
    let _ = stdout().flush();
    let mut cid = String::new();
    let _ = stdin().read_line(&mut cid);
    let client_id = cid.trim().to_string();

    println!("Enter client_secret (or leave blank if not needed): ");
    print!("> ");
    let _ = stdout().flush();
    let mut csec = String::new();
    let _ = stdin().read_line(&mut csec);
    let client_secret = csec.trim().to_string();
    let secret_opt = if client_secret.is_empty() { None } else { Some(client_secret) };

    // Now call the API
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let result = rt.block_on(bot_api.create_platform_config(
        platform_str.clone().parse().unwrap(),
        &label,
        client_id,
        secret_opt,
    ));
    match result {
        Ok(_) => format!("Platform config added for platform='{}', label='{}'.", platform_str, label),
        Err(e) => format!("Error => {:?}", e),
    }
}

/// For removing, we might ask for an ID from the user, or list them first.
fn handle_platform_remove(bot_api: &Arc<dyn BotApi>) -> String {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let list = match rt.block_on(bot_api.list_platform_configs(None)) {
        Ok(lst) => lst,
        Err(e) => {
            return format!("Error listing platform configs => {:?}", e);
        }
    };

    if list.is_empty() {
        return "No platform configs to remove.".to_string();
    }
    println!("Existing platform configs:");
    for pc in &list {
        println!(" - id={} platform={} label={}",
                 pc.platform_config_id, pc.platform, pc.platform_label.as_deref().unwrap_or(""));
    }
    println!("Enter the platform_config_id to remove: ");
    print!("> ");
    let _ = stdout().flush();

    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let chosen_id = line.trim().to_string();
    if chosen_id.is_empty() {
        return "Aborted removal (no input).".to_string();
    }

    // remove by ID:
    match rt.block_on(bot_api.remove_platform_config(&chosen_id)) {
        Ok(_) => format!("Removed platform config with id={}.", chosen_id),
        Err(e) => format!("Error removing => {:?}", e),
    }
}