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
        return "Usage: account <add|remove|list|show> [platform] [username]".to_string();
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
            // optional filter
            let maybe_platform = if args.len() > 1 {
                Platform::from_str(args[1]).ok()
            } else {
                None
            };

            // ----------------------------------
            // FIX: Use a multi-threaded runtime
            // ----------------------------------
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();

            match rt.block_on(bot_api.list_credentials(maybe_platform)) {
                Ok(list) => {
                    if list.is_empty() {
                        "No stored platform credentials.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Stored platform credentials:\n");
                        for c in list {
                            out.push_str(&format!(
                                " - user_id={} platform={:?} is_bot={} credential_id={}\n",
                                c.user_id, c.platform, c.is_bot, c.credential_id
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "show" => {
            // account show <platform> <username>
            if args.len() < 3 {
                return "Usage: account show <platform> <username>".to_string();
            }
            let platform_str = args[1];
            let username = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_show(p, username, bot_api),
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        _ => "Usage: account <add|remove|list|show> [platform] [username]".to_string(),
    }
}

/// The 2-step OAuth flow for adding credentials:
/// Step 1 => begin_auth_flow => we open in browser
/// Step 2 => wait for callback => complete_auth_flow_for_user => store in DB
fn account_add_flow(platform: Platform, username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    println!("Is this a bot account? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    // ----------------------------------
    // FIX: Also multi-threaded here
    // ----------------------------------
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // 1) Start local callback server on port=9876
    let fixed_port: u16 = 9876;
    let (done_rx, shutdown_tx) = match rt.block_on(start_callback_server(fixed_port)) {
        Ok(pair) => pair,
        Err(e) => return format!("Error starting callback server => {:?}", e),
    };

    // 2) Begin auth flow
    let url_res = rt.block_on(bot_api.begin_auth_flow(platform.clone(), is_bot));
    let url = match url_res {
        Ok(u) => u,
        Err(e) => {
            shutdown_tx.send(()).ok();
            return format!("Error => {:?}", e);
        }
    };

    println!("2025-02-10T01:06:28.902887Z  INFO maowbot_core::auth::callback_server: OAuth callback server listening on http://127.0.0.1:{}", fixed_port);
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

    // 3) Wait for OAuth callback
    let callback_result = match done_rx.blocking_recv() {
        Ok(res) => res,
        Err(e) => {
            shutdown_tx.send(()).ok();
            return format!("Error receiving OAuth code => {:?}", e);
        }
    };
    // Shut down the local callback server
    shutdown_tx.send(()).ok();

    // 4) Complete the auth flow with user_id=the TUI `username`
    match rt.block_on(bot_api.complete_auth_flow_for_user(
        platform.clone(),
        callback_result.code,
        username,
    )) {
        Ok(cred) => {
            format!(
                "Success! Stored credentials for platform={:?}, user_id='{}', is_bot={}.",
                cred.platform, cred.user_id, cred.is_bot
            )
        }
        Err(e) => format!("Error completing auth => {:?}", e),
    }
}

/// Revoke stored credentials
fn account_remove(platform: Platform, username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // --------------------------------
    // Switch from current_thread to multi_thread:
    // --------------------------------
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    match rt.block_on(bot_api.revoke_credentials(platform.clone(), username)) {
        Ok(_) => format!("Removed credentials for platform={:?}, user_id={}", platform, username),
        Err(e) => format!("Error removing => {:?}", e),
    }
}

/// "account show <platform> <username>"
fn account_show(platform: Platform, username: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // --------------------------------
    // Also multi‐threaded
    // --------------------------------
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // We'll re-use list_credentials(Some(platform)), then filter by user_id:
    let all = match rt.block_on(bot_api.list_credentials(Some(platform.clone()))) {
        Ok(list) => list,
        Err(e) => return format!("Error => {:?}", e),
    };

    let maybe_cred = all.into_iter().find(|c| c.user_id == username);
    match maybe_cred {
        Some(c) => {
            let mut out = String::new();
            out.push_str(&format!("platform={:?}\nuser_id={}\n", c.platform, c.user_id));
            out.push_str(&format!("credential_type={:?}\nis_bot={}\n", c.credential_type, c.is_bot));
            out.push_str(&format!("primary_token='{}'\n", c.primary_token));
            out.push_str(&format!("refresh_token='{:?}'\n", c.refresh_token));
            out.push_str(&format!("additional_data={:?}\n", c.additional_data));
            out.push_str(&format!("expires_at={:?}\n", c.expires_at));
            out.push_str(&format!("created_at={}\nupdated_at={}\n", c.created_at, c.updated_at));
            out
        }
        None => format!("No credentials found for platform={:?}, user_id='{}'", platform, username),
    }
}