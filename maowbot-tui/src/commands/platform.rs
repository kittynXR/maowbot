// =============================================================================
// maowbot-tui/src/commands/platform.rs
//   - Adds "platform show <platform>" to display the stored client_id/secret.
// =============================================================================

use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use std::str::FromStr;

/// platform <add|remove|list|show> ...
/// Usage examples:
///   platform add twitch
///   platform remove
///   platform list
///   platform show twitch
pub fn handle_platform_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: platform <add|remove|list|show> ...".to_string();
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
            handle_platform_remove(bot_api)
        }
        "list" => {
            let maybe_platform = if args.len() > 1 {
                Some(args[1])
            } else {
                None
            };
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
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
                                " - id={} platform={} client_id={}\n",
                                pc.platform_config_id,
                                pc.platform,
                                pc.client_id.as_deref().unwrap_or("NONE"),
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing platform configs => {:?}", e),
            }
        }
        "show" => {
            if args.len() < 2 {
                return "Usage: platform show <platformName>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(plat) => platform_show(plat, bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        _ => "Usage: platform <add|remove|list|show>".to_string(),
    }
}

fn handle_platform_add(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    println!("You are adding or updating the single platform config for '{}'.", platform_str);

    // Possibly prompt user to open the dev console if we know it:
    let dev_console_url = match platform_str.as_str() {
        "twitch" => Some("https://dev.twitch.tv/console"),
        "discord" => Some("https://discord.com/developers/applications"),
        "vrchat" => Some("https://dashboard.vrchat.com/"),
        "twitch-irc" => None,
        _ => None,
    };
    if let Some(url) = dev_console_url {
        println!("Open the dev console for {} now? (y/n):", platform_str);
        print!("> ");
        let _ = stdout().flush();
        let mut line = String::new();
        let _ = stdin().read_line(&mut line);
        if line.trim().eq_ignore_ascii_case("y") {
            let _ = open::that(url);
        }
    }

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

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let result = rt.block_on(bot_api.create_platform_config(
        plat,
        client_id,
        secret_opt,
    ));
    match result {
        Ok(_) => format!("Platform config upserted for platform='{}'.", platform_str),
        Err(e) => format!("Error => {:?}", e),
    }
}

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
        println!(" - id={} platform={} client_id={}",
                 pc.platform_config_id,
                 pc.platform,
                 pc.client_id.as_deref().unwrap_or("NONE"));
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

    match rt.block_on(bot_api.remove_platform_config(&chosen_id)) {
        Ok(_) => format!("Removed platform config with id={}.", chosen_id),
        Err(e) => format!("Error removing => {:?}", e),
    }
}

/// "platform show <platformName>"
fn platform_show(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    // list_platform_configs(Some("twitch")) => we should get at most 1 row for that platform
    let result = rt.block_on(bot_api.list_platform_configs(Some(&platform_str)));
    let list = match result {
        Ok(lst) => lst,
        Err(e) => return format!("Error => {:?}", e),
    };
    if list.is_empty() {
        return format!("No platform config found for '{}'.", platform_str);
    }

    // We expect only one row per platform. Show it.
    let pc = &list[0];
    let mut out = String::new();
    out.push_str(&format!("platform={} (id={})\n", pc.platform, pc.platform_config_id));
    out.push_str(&format!("client_id='{}'\n", pc.client_id.as_deref().unwrap_or("NONE")));
    out.push_str(&format!("client_secret='{}'\n",
                          pc.client_secret.as_deref().unwrap_or("NONE")));
    out
}