// maowbot-tui/src/commands/account.rs

use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::str::FromStr;
use open;
use maowbot_core::models::Platform;
use maowbot_core::auth::callback_server::start_callback_server;
use maowbot_core::plugins::bot_api::BotApi;

pub fn handle_account_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: account <add|remove|list> [platform] [username]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 3 {
                return "Usage: account add <platform> <username>".to_string();
            }
            let platform_str = args[1];
            let username = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_add_flow(p, username, bot_api),
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "remove" => {
            if args.len() < 3 {
                return "Usage: account remove <platform> <username>".to_string();
            }
            let platform_str = args[1];
            let username = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_remove(p, username, bot_api),
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "list" => {
            // optional: if user typed `account list twitch` then we filter
            let maybe_platform = if args.len() > 1 {
                Platform::from_str(args[1]).ok()
            } else {
                None
            };
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            match rt.block_on(bot_api.list_credentials(maybe_platform)) {
                Ok(list) => {
                    if list.is_empty() {
                        "No stored platform credentials.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Stored platform credentials:\n");
                        for c in list {
                            out.push_str(&format!(
                                " - user_id={} platform={:?} is_bot={}\n",
                                c.user_id, c.platform, c.is_bot
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error => {:?}", e),
            }
        }
        _ => "Usage: account <add|remove|list> [platform] [username]".to_string(),
    }
}

/// The actual 2-step OAuth or token flow for adding credentials.
fn account_add_flow(platform: Platform, username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // 1) Ask if it's a bot account
    println!("Is this a bot account? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    // 2) We want to do the OAuth flow with some label that can be the "username" or something else
    //    For simplicity, let's just re-use `username` as the label, or the user can confirm:
    println!("Use '{}' as the label? (y/n)", username);
    print!("> ");
    let _ = stdout().flush();
    let mut label_line = String::new();
    let _ = stdin().read_line(&mut label_line);
    let final_label = if label_line.trim().eq_ignore_ascii_case("y") {
        username.to_string()
    } else {
        // prompt for custom label:
        println!("Enter custom label:");
        print!("> ");
        let _ = stdout().flush();
        let mut line2 = String::new();
        let _ = stdin().read_line(&mut line2);
        line2.trim().to_string()
    };

    if final_label.is_empty() {
        return "Aborted (no label).".to_string();
    }

    // 3) Start local callback server on port=9876
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let fixed_port: u16 = 9876;
    let (done_rx, shutdown_tx) = match rt.block_on(start_callback_server(fixed_port)) {
        Ok(pair) => pair,
        Err(e) => return format!("Error starting callback server => {:?}", e),
    };

    // 4) Begin auth flow
    let url_res = rt.block_on(bot_api.begin_auth_flow_with_label(platform.clone(), is_bot, &final_label));
    let url = match url_res {
        Ok(u) => u,
        Err(e) => {
            shutdown_tx.send(()).ok();
            return format!("Error => {:?}", e);
        }
    };

    println!("Open this URL to authenticate:\n  {}", url);
    println!("Open in browser now? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line2 = String::new();
    let _ = stdin().read_line(&mut line2);
    if line2.trim().eq_ignore_ascii_case("y") {
        let _ = open::that(&url);
    }
    println!("Waiting for the OAuth callback on port {}...", fixed_port);

    // 5) Wait for callback
    let callback_result = match done_rx.blocking_recv() {
        Ok(res) => res,
        Err(e) => {
            shutdown_tx.send(()).ok();
            return format!("Error receiving OAuth code => {:?}", e);
        }
    };
    shutdown_tx.send(()).ok();

    // 6) Complete
    match rt.block_on(bot_api.complete_auth_flow(platform.clone(), callback_result.code)) {
        Ok(cred) => {
            format!(
                "Success! Stored credentials for platform={:?}, user_id='{}', is_bot={}, label='{}'",
                cred.platform, cred.user_id, cred.is_bot, final_label
            )
        }
        Err(e) => {
            format!("Error completing auth => {:?}", e)
        }
    }
}

/// Revoke stored credentials
fn account_remove(platform: Platform, username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(bot_api.revoke_credentials(platform.clone(), username)) {
        Ok(_) => format!("Removed credentials for platform={:?}, user_id={}", platform, username),
        Err(e) => format!("Error removing => {:?}", e),
    }
}