// File: maowbot-tui/src/commands/account.rs

use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::str::FromStr;
use open;
use tokio::runtime::Builder as RuntimeBuilder;

use maowbot_core::models::{Platform, User};
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::auth::callback_server::start_callback_server;
use maowbot_core::Error;

/// Handle "account <add|remove|list|show>" commands.
pub fn handle_account_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: account <add|remove|list|show> [platform] [username]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 3 {
                return "Usage: account add <platform> <desired_global_username>".to_string();
            }
            let platform_str = args[1];
            let typed_name   = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_add_flow(p, typed_name, bot_api),
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "remove" => {
            if args.len() < 3 {
                return "Usage: account remove <platform> <usernameOrUserId>".to_string();
            }
            let platform_str = args[1];
            let user_id_str  = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_remove(p, user_id_str, bot_api),
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
            let rt = RuntimeBuilder::new_current_thread()
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
            // account show <platform> <usernameOrUserId>
            if args.len() < 3 {
                return "Usage: account show <platform> <userId>".to_string();
            }
            let platform_str = args[1];
            let user_id_str  = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_show(p, user_id_str, bot_api),
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        _ => "Usage: account <add|remove|list|show> [platform] [username]".to_string(),
    }
}

/// The “account add” flow (OAuth2 or other method):
///
/// 1) Prompt “Is this a bot account?”
/// 2) Ask user if they want to keep the typed name (`typed_name`) as a new global username or pick a different one.
///    Then find/create a user row (which yields a real user_id=UUID).
/// 3) Optionally ask about the platform-specific user ID, if that matters.
/// 4) Start auth flow -> open browser -> wait for callback -> complete auth -> store credentials for that user_id.
fn account_add_flow(platform: Platform, typed_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // is bot?
    println!("Is this a bot account? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    // Ask: “Use ‘{typed_name}’ for the user’s global_username in the DB?”
    println!("Use '{}' for the user’s global_username? (y/n):", typed_name);
    print!("> ");
    let _ = stdout().flush();
    let mut line2 = String::new();
    let _ = stdin().read_line(&mut line2);
    let mut final_username = typed_name.to_string();
    if !line2.trim().eq_ignore_ascii_case("y") {
        println!("Enter a different global username:");
        print!("> ");
        let _ = stdout().flush();
        let mut alt = String::new();
        let _ = stdin().read_line(&mut alt);
        let alt = alt.trim();
        if !alt.is_empty() {
            final_username = alt.to_string();
        }
    }

    // Create a small runtime:
    let rt = RuntimeBuilder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // Step A: Check if there's already a user with that global_username, or create one:
    let user = match rt.block_on(find_or_create_user_by_name(bot_api, &final_username)) {
        Ok(u) => u,
        Err(e) => return format!("Error finding/creating user '{}': {:?}", final_username, e),
    };

    println!(
        "Will associate new credentials with user_id={}, global_username='{}'",
        user.user_id, user.global_username.as_deref().unwrap_or("")
    );

    // Step B: Start the local callback server on a fixed port:
    let port = 9876;
    let (done_rx, shutdown_tx) = match rt.block_on(start_callback_server(port)) {
        Ok(pair) => pair,
        Err(e) => {
            return format!("Error starting callback server => {:?}", e);
        }
    };

    println!(
        "OAuth callback server listening on http://127.0.0.1:{}",
        port
    );

    // Step C: begin auth flow
    let url_res = rt.block_on(bot_api.begin_auth_flow(platform.clone(), is_bot));
    let url = match url_res {
        Ok(u) => u,
        Err(e) => {
            let _ = shutdown_tx.send(());
            return format!("Error => {:?}", e);
        }
    };

    println!("Open this URL to authenticate:\n  {}", url);
    println!("Open in browser now? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line3 = String::new();
    let _ = stdin().read_line(&mut line3);
    if line3.trim().eq_ignore_ascii_case("y") {
        let _ = open::that(&url);
    }
    println!("Waiting for the OAuth callback on port {}...", port);

    // Step D: wait for the callback
    let callback_result = match done_rx.blocking_recv() {
        Ok(res) => res,
        Err(e) => {
            let _ = shutdown_tx.send(());
            return format!("Error receiving OAuth code => {:?}", e);
        }
    };
    // Shut down the local callback server now that we have our code
    let _ = shutdown_tx.send(());

    // Step E: complete the auth flow with the real user_id
    match rt.block_on(bot_api.complete_auth_flow_for_user(
        platform.clone(),
        callback_result.code,
        user.user_id,
    )) {
        Ok(cred) => {
            format!(
                "Success! Stored credentials => platform={:?}, user_id={}, is_bot={}",
                cred.platform,
                cred.user_id,
                cred.is_bot
            )
        }
        Err(e) => format!("Error completing auth => {:?}", e),
    }
}

/// Helper that tries to find a user with `global_username == name`, or if not found, creates one.
/// Returns the full user record (including the real user_id=UUID).
async fn find_or_create_user_by_name(
    bot_api: &Arc<dyn BotApi>,
    final_username: &str
) -> Result<User, Error> {
    // Example: call an async method to search
    let all = bot_api.search_users(final_username).await?;
    if let Some(u) = all.into_iter().find(|usr| {
        usr.global_username.as_deref() == Some(final_username)
    }) {
        // found
        Ok(u)
    } else {
        // not found => create
        use uuid::Uuid;
        let new_uuid = Uuid::new_v4();

        // also an async call
        bot_api.create_user(new_uuid, final_username).await?;

        // fetch or build a new user object to return
        // e.g. get_user(new_uuid)
        let user_opt = bot_api.get_user(new_uuid).await?;
        let user = user_opt.ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;
        Ok(user)
    }
}

fn account_remove(platform: Platform, user_id_str: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = RuntimeBuilder::new_current_thread().enable_all().build().unwrap();
    match rt.block_on(bot_api.revoke_credentials(platform.clone(), user_id_str.parse().unwrap())) {
        Ok(_) => format!("Removed credentials for platform={:?}, user_id={}", platform, user_id_str),
        Err(e) => format!("Error removing => {:?}", e),
    }
}

fn account_show(platform: Platform, user_id_str: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = RuntimeBuilder::new_current_thread().enable_all().build().unwrap();

    // We'll re-use list_credentials(Some(platform)), then filter by user_id:
    let all = match rt.block_on(bot_api.list_credentials(Some(platform.clone()))) {
        Ok(list) => list,
        Err(e) => return format!("Error => {:?}", e),
    };

    // Compare user_id_str to credential.user_id.to_string()
    let maybe_cred = all.into_iter().find(|c| c.user_id.to_string() == user_id_str);
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
        None => format!(
            "No credentials found for platform={:?}, user_id='{}'",
            platform, user_id_str
        ),
    }
}