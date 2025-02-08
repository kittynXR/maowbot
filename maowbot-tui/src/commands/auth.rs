// maowbot-tui/src/commands/auth.rs
//
// This updated file adds a prompt for the user to choose or override
// an auth "label," checks if there's an existing auth_config row
// for that (platform, label), and if not found, prompts for client_id
// and client_secret, creates a new row, and retries the flow.
//
// Now multiple client_ids can be stored by using different labels,
// e.g. “bot1” or “user2,” etc.

use std::str::FromStr;
use std::sync::Arc;
use open;
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::error::Error as CoreError;

pub fn handle_auth_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: auth <add|remove|list> [platform] [user_id]".to_string();
    }
    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: auth add <platform>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(p) => auth_add_flow(p, bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "remove" => {
            if args.len() < 3 {
                return "Usage: auth remove <platform> <user_id>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(p) => auth_remove(p, args[2], bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "list" => {
            let maybe_platform = if args.len() > 1 {
                Platform::from_str(args[1]).ok()
            } else {
                None
            };
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let result = rt.block_on(bot_api.list_credentials(maybe_platform));
            match result {
                Ok(creds) => {
                    let mut s = String::new();
                    s.push_str("Stored credentials:\n");
                    for c in creds {
                        s.push_str(&format!(
                            " - user_id={} platform={:?} is_bot={} label_in_repo?\n",
                            c.user_id, c.platform, c.is_bot
                        ));
                    }
                    s
                }
                Err(e) => format!("Error listing credentials => {:?}", e),
            }
        }
        _ => "Usage: auth <add|remove|list> [platform] [user_id]".to_string(),
    }
}

/// Interactive ‘auth add’ flow:
/// 1) Ask if this is a bot
/// 2) Generate or ask for a "label" to store in the auth_config table
/// 3) Attempt begin_auth_flow_with_label. If row not found, prompt for new client_id/secret, create row.
/// 4) Then open browser / handle code, finalize flow.
fn auth_add_flow(platform: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    println!("Is this a bot account? (y/n):");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    // Prompt user to pick a label
    let label = prompt_for_label(&platform, is_bot, bot_api);

    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return format!("Error creating tokio runtime: {:?}", e),
    };

    // Step 1: begin auth with this label
    let url_result = rt.block_on(bot_api.begin_auth_flow_with_label(platform.clone(), is_bot, &label));
    let url = match url_result {
        Ok(u) => u,
        Err(CoreError::Auth(msg)) if msg.contains("No auth_config row found") => {
            // This means there's no row. Prompt user to create one:
            println!("No config found for (platform={:?}, label='{}').", platform, label);
            println!("Let's create a new client_id / client_secret for it now.\n");

            // Not all platforms require a secret, but let's do it generally:
            let client_id = prompt("Enter client_id:");
            let client_secret = prompt("Enter client_secret (or leave blank if not needed):");
            let create_res = rt.block_on(
                bot_api.create_auth_config(platform.clone(), &label, client_id,
                                           if client_secret.trim().is_empty() { None } else { Some(client_secret) }
                )
            );
            if let Err(e) = create_res {
                return format!("Error creating new auth_config => {:?}", e);
            }

            // Now that we have a row, try again:
            match rt.block_on(bot_api.begin_auth_flow_with_label(platform.clone(), is_bot, &label)) {
                Ok(u) => u,
                Err(e) => return format!("Error beginning auth flow after creation => {:?}", e),
            }
        }
        Err(e) => {
            return format!("Error beginning auth flow => {:?}", e);
        }
    };

    // We have a valid URL for OAuth or something. Prompt user to open it:
    println!("Open this URL to authenticate:\n  {}", url);
    println!("Open in browser now? (y/n):");
    let mut line2 = String::new();
    let _ = std::io::stdin().read_line(&mut line2);
    if line2.trim().eq_ignore_ascii_case("y") {
        if let Err(err) = open::that(&url) {
            println!("Could not open browser automatically: {:?}", err);
        }
    }

    println!("If you were given a 'code=' param in the callback (or it auto-redirected), enter it here.\n(Press enter if no manual code): ");
    let mut code_line = String::new();
    let _ = std::io::stdin().read_line(&mut code_line);
    let code_str = code_line.trim().to_string();

    // Step 2: complete the flow
    match rt.block_on(bot_api.complete_auth_flow(platform, code_str)) {
        Ok(cred) => {
            format!("Success! Stored credentials for platform={:?}, is_bot={}, label='{}'",
                    cred.platform, cred.is_bot, label)
        }
        Err(e) => {
            format!("Error completing auth => {:?}", e)
        }
    }
}

fn auth_remove(platform: Platform, user_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return format!("Error creating tokio runtime: {:?}", e),
    };

    match rt.block_on(bot_api.revoke_credentials(platform.clone(), user_id)) {
        Ok(_) => {
            format!("Removed credentials for platform={:?}, user_id={}", platform, user_id)
        }
        Err(e) => {
            format!("Error removing credentials => {:?}", e)
        }
    }
}

/// Prompts user to generate or override a default label (bot1, bot2, user1, etc).
fn prompt_for_label(platform: &Platform, is_bot: bool, bot_api: &Arc<dyn BotApi>) -> String {
    // We'll count how many existing rows for that platform to guess a default
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let platform_str = format!("{}", platform);
    let count_res = rt.block_on(bot_api.count_auth_configs_for_platform(platform_str.clone()));
    let current_count = match count_res {
        Ok(n) => n,
        Err(_) => 0,
    };

    let proposed = if is_bot {
        format!("bot{}", current_count + 1)
    } else {
        format!("user{}", current_count + 1)
    };

    println!("Proposed label='{}'. Use this? (y/n)", proposed);
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    if line.trim().eq_ignore_ascii_case("y") {
        proposed
    } else {
        prompt("Enter custom label:")
    }
}

/// Simple helper to prompt user for a single line of input
fn prompt(msg: &str) -> String {
    println!("{}", msg);
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_string()
}