// File: maowbot-tui/src/commands/account.rs

use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::collections::HashMap;
use std::str::FromStr;
use open;
use uuid::Uuid;

use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::Error;

/// Handle "account <add|remove|list|show>" commands asynchronously.
pub async fn handle_account_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: account <add|remove|list|show> [platform] [username|UUID]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 3 {
                return "Usage: account add <platform> <desired_global_username>".to_string();
            }
            let platform_str = args[1];
            let typed_name   = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_add_flow(p, typed_name, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "remove" => {
            if args.len() < 3 {
                return "Usage: account remove <platform> <usernameOrUUID>".to_string();
            }
            let platform_str = args[1];
            let user_id_str  = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_remove(p, user_id_str, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "list" => {
            // optional filter: "account list <platform>"
            let maybe_platform = if args.len() > 1 {
                Platform::from_str(args[1]).ok()
            } else {
                None
            };
            account_list(maybe_platform, bot_api).await
        }
        "show" => {
            if args.len() < 3 {
                return "Usage: account show <platform> <usernameOrUUID>".to_string();
            }
            let platform_str = args[1];
            let user_id_str  = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_show(p, user_id_str, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        _ => "Usage: account <add|remove|list|show> [platform] [username|UUID]".to_string(),
    }
}

async fn account_add_flow(platform: Platform, typed_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    println!("Is this a bot account? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

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

    // Step A: find or create user
    let user = match find_or_create_user_by_name(bot_api, &final_username).await {
        Ok(u) => u,
        Err(e) => return format!("Error finding/creating user '{}': {:?}", final_username, e),
    };
    println!(
        "Will associate new credentials with user_id={}, global_username='{}'",
        user.user_id,
        user.global_username.as_deref().unwrap_or("(none)")
    );

    // Step B: Begin the auth flow
    let flow_str = match bot_api.begin_auth_flow(platform.clone(), is_bot).await {
        Ok(u) => u,
        Err(e) => return format!("Error => {:?}", e),
    };

    if flow_str.starts_with("http://") || flow_str.starts_with("https://") {
        return handle_oauth_browser_flow(flow_str, platform, user.user_id, bot_api).await;
    }
    if flow_str.contains("(Multiple keys required)") {
        return handle_multiple_keys_flow(platform, user.user_id, bot_api).await;
    }
    if flow_str.contains("(API key)") {
        return handle_api_key_flow(flow_str, platform, user.user_id, bot_api).await;
    }
    if flow_str.contains("(2FA)") {
        return "(2FA) not yet implemented in TUI example".to_string();
    }
    if flow_str.contains("(No prompt needed)") {
        return "(No prompt needed) Possibly done?".to_string();
    }

    flow_str
}

async fn handle_oauth_browser_flow(
    auth_url: String,
    platform: Platform,
    user_id: Uuid,
    bot_api: &Arc<dyn BotApi>
) -> String {
    println!("Open this URL to authenticate:\n  {}", auth_url);
    println!("Open in browser now? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line3 = String::new();
    let _ = stdin().read_line(&mut line3);
    if line3.trim().eq_ignore_ascii_case("y") {
        let _ = open::that(&auth_url);
    }

    // Start callback server
    let start_res = maowbot_core::auth::callback_server::start_callback_server(9876);
    let (done_rx, shutdown_tx) = match start_res.await {
        Ok(pair) => pair,
        Err(e) => {
            return format!("Error starting callback server => {:?}", e);
        }
    };
    println!("OAuth callback server listening on http://127.0.0.1:9876");
    println!("Waiting for the OAuth callback...");

    let callback_result = match done_rx.await {
        Ok(r) => r,
        Err(e) => {
            let _ = shutdown_tx.send(());
            return format!("Error receiving OAuth code => {:?}", e);
        }
    };
    let _ = shutdown_tx.send(());

    let cred_res = bot_api
        .complete_auth_flow_for_user(platform, callback_result.code, user_id)
        .await;
    match cred_res {
        Ok(cred) => {
            format!(
                "Success! Stored credentials => platform={:?}, user_id={}, is_bot={}",
                cred.platform, cred.user_id, cred.is_bot
            )
        }
        Err(e) => format!("Error completing auth => {:?}", e),
    }
}

async fn handle_multiple_keys_flow(
    platform: Platform,
    user_id: Uuid,
    bot_api: &Arc<dyn BotApi>
) -> String {
    println!("Enter each required field (example: 'bot_token'):");
    let mut keys_map = HashMap::new();

    loop {
        print!("Key name (empty to finish) > ");
        let _ = stdout().flush();
        let mut keyname = String::new();
        if stdin().read_line(&mut keyname).is_err() {
            return "Error reading key name".to_string();
        }
        let keyname = keyname.trim();
        if keyname.is_empty() {
            break;
        }

        print!("Value for '{keyname}'> ");
        let _ = stdout().flush();
        let mut val = String::new();
        if stdin().read_line(&mut val).is_err() {
            return "Error reading key value".to_string();
        }
        let val = val.trim().to_string();

        keys_map.insert(keyname.to_string(), val);
    }

    if keys_map.is_empty() {
        return "No keys entered. Aborting.".to_string();
    }

    match bot_api
        .complete_auth_flow_for_user_multi(platform, user_id, keys_map)
        .await
    {
        Ok(cred) => format!(
            "Success! Stored credentials => platform={:?}, user_id={}, is_bot={}",
            cred.platform, cred.user_id, cred.is_bot
        ),
        Err(e) => format!("Error completing multi-key auth => {:?}", e),
    }
}

async fn handle_api_key_flow(
    prompt_msg: String,
    platform: Platform,
    user_id: Uuid,
    bot_api: &Arc<dyn BotApi>
) -> String {
    println!("Auth flow said: {}", prompt_msg);

    print!("Paste the API key now:\n> ");
    let _ = stdout().flush();
    let mut key_line = String::new();
    let _ = stdin().read_line(&mut key_line);
    let token_str = key_line.trim().to_string();
    if token_str.is_empty() {
        return "No API key entered. Aborting.".to_string();
    }
    let mut map = HashMap::new();
    map.insert("api_key".into(), token_str);

    match bot_api
        .complete_auth_flow_for_user_multi(platform, user_id, map)
        .await
    {
        Ok(cred) => format!(
            "Success! Stored credentials => platform={:?}, user_id={}, is_bot={}",
            cred.platform, cred.user_id, cred.is_bot
        ),
        Err(e) => format!("Error completing single-key flow => {:?}", e),
    }
}

// find or create user by name
async fn find_or_create_user_by_name(
    bot_api: &Arc<dyn BotApi>,
    final_username: &str
) -> Result<maowbot_core::models::User, Error> {
    let all = bot_api.search_users(final_username).await?;
    if let Some(u) = all.into_iter().find(|usr| {
        usr.global_username.as_deref().map(|s| s.to_lowercase())
            == Some(final_username.to_lowercase())
    }) {
        Ok(u)
    } else {
        let new_uuid = Uuid::new_v4();
        bot_api.create_user(new_uuid, final_username).await?;
        let user_opt = bot_api.get_user(new_uuid).await?;
        let user = user_opt.ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;
        Ok(user)
    }
}

async fn account_remove(platform: Platform, user_id_str: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let user_uuid = match Uuid::parse_str(user_id_str) {
        Ok(u) => u,
        Err(_) => {
            // find user by name
            match bot_api.find_user_by_name(user_id_str).await {
                Ok(u) => u.user_id,
                Err(e) => {
                    return format!("No user found with name '{}': {:?}", user_id_str, e);
                }
            }
        }
    };

    match bot_api.revoke_credentials(platform.clone(), user_uuid.to_string()).await {
        Ok(_) => format!(
            "Removed credentials for platform='{:?}', user_id={}",
            platform, user_uuid
        ),
        Err(e) => format!("Error removing => {:?}", e),
    }
}

async fn account_list(
    maybe_platform: Option<Platform>,
    bot_api: &Arc<dyn BotApi>
) -> String {
    match bot_api.list_credentials(maybe_platform).await {
        Ok(list) => {
            if list.is_empty() {
                "No stored platform credentials.\n".to_string()
            } else {
                let mut out = String::new();
                out.push_str("Stored platform credentials:\n");
                for c in list {
                    let username_or_id = match bot_api.get_user(c.user_id).await {
                        Ok(Some(u)) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
                        _ => c.user_id.to_string(),
                    };
                    out.push_str(&format!(
                        " - user='{}' platform={:?} is_bot={} credential_id={}\n",
                        username_or_id, c.platform, c.is_bot, c.credential_id
                    ));
                }
                out
            }
        }
        Err(e) => format!("Error => {:?}", e),
    }
}

async fn account_show(
    platform: Platform,
    user_id_str: &str,
    bot_api: &Arc<dyn BotApi>
) -> String {
    let user_uuid = match Uuid::parse_str(user_id_str) {
        Ok(u) => u,
        Err(_) => {
            // find user
            match bot_api.find_user_by_name(user_id_str).await {
                Ok(u) => u.user_id,
                Err(_) => {
                    return format!(
                        "No credentials found for platform={:?}, user='{}'",
                        platform, user_id_str
                    );
                }
            }
        }
    };

    let all = match bot_api.list_credentials(Some(platform.clone())).await {
        Ok(list) => list,
        Err(e) => return format!("Error => {:?}", e),
    };

    if let Some(c) = all.into_iter().find(|cred| cred.user_id == user_uuid) {
        let mut out = String::new();
        out.push_str(&format!("platform={:?}\n", c.platform));
        out.push_str(&format!("user_id={}\n", c.user_id));
        out.push_str(&format!("credential_type={:?}\n", c.credential_type));
        out.push_str(&format!("is_bot={}\n", c.is_bot));
        out.push_str(&format!("primary_token='{}'\n", c.primary_token));
        let refresh_str = c.refresh_token.as_deref().unwrap_or("(none)");
        out.push_str(&format!("refresh_token='{}'\n", refresh_str));
        out.push_str(&format!("additional_data={:?}\n", c.additional_data));
        out.push_str(&format!("expires_at={:?}\n", c.expires_at));
        out.push_str(&format!("created_at={}\nupdated_at={}\n", c.created_at, c.updated_at));
        out
    } else {
        format!("No credentials found for platform={:?}, user_id='{}'", platform, user_uuid)
    }
}