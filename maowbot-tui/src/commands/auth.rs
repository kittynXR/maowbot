// maowbot-tui/src/commands/auth.rs

use std::str::FromStr;
use open;
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use std::sync::Arc;

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
                            " - user_id={} platform={:?} is_bot={}\n",
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

fn auth_add_flow(platform: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    println!("Is this a bot account? (y/n):");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return format!("Error creating tokio runtime: {:?}", e),
    };

    // Step 1: begin auth
    let url_result = rt.block_on(bot_api.begin_auth_flow(platform.clone(), is_bot));
    let url = match url_result {
        Ok(u) => u,
        Err(e) => return format!("Error beginning auth flow => {:?}", e),
    };

    println!("Open this URL to authenticate:\n  {}", url);
    println!("Open in browser now? (y/n):");
    let mut line2 = String::new();
    let _ = std::io::stdin().read_line(&mut line2);
    if line2.trim().eq_ignore_ascii_case("y") {
        if let Err(err) = open::that(&url) {
            println!("Could not open browser automatically: {:?}", err);
        }
    }

    println!("If a 'code' param was displayed, enter it here (or just press Enter if code is auto-handled): ");
    let mut code_line = String::new();
    let _ = std::io::stdin().read_line(&mut code_line);
    let code_str = code_line.trim().to_string();

    // Step 2: complete the flow
    match rt.block_on(bot_api.complete_auth_flow(platform, code_str)) {
        Ok(cred) => {
            format!("Success! Stored credentials for platform={:?}, is_bot={}", cred.platform, cred.is_bot)
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