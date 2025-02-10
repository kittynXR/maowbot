use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::collections::HashMap;
use std::str::FromStr;
use open;
use uuid::Uuid;

use maowbot_core::models::{Platform, User};
use maowbot_core::auth::callback_server::start_callback_server;
use maowbot_core::Error;
use crate::tui_module::tui_block_on;
use maowbot_core::plugins::bot_api::BotApi;

/// Handle "account <add|remove|list|show>" commands.
pub fn handle_account_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
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
                Ok(p) => account_add_flow(p, typed_name, bot_api),
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
                Ok(p) => account_remove(p, user_id_str, bot_api),
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
            account_list(maybe_platform, bot_api)
        }
        "show" => {
            // account show <platform> <usernameOrUUID>
            if args.len() < 3 {
                return "Usage: account show <platform> <usernameOrUUID>".to_string();
            }
            let platform_str = args[1];
            let user_id_str  = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_show(p, user_id_str, bot_api),
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        _ => "Usage: account <add|remove|list|show> [platform] [username|UUID]".to_string(),
    }
}

/// The “account add” flow (OAuth2 or multiple keys):
fn account_add_flow(platform: Platform, typed_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Ask if it's a bot or user account
    println!("Is this a bot account? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    // Let them confirm the global username
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
    let user = match tui_block_on(find_or_create_user_by_name(bot_api, &final_username)) {
        Ok(u) => u,
        Err(e) => return format!("Error finding/creating user '{}': {:?}", final_username, e),
    };
    println!(
        "Will associate new credentials with user_id={}, global_username='{}'",
        user.user_id,
        user.global_username.as_deref().unwrap_or("(none)")
    );

    // Step B: Begin the auth flow
    let flow_str = match tui_block_on(bot_api.begin_auth_flow(platform.clone(), is_bot)) {
        Ok(u) => u,
        Err(e) => return format!("Error => {:?}", e),
    };

    // If the flow_str is "http..." (Browser) → do our callback approach:
    if flow_str.starts_with("http://") || flow_str.starts_with("https://") {
        return handle_oauth_browser_flow(flow_str, platform, &user.user_id, bot_api);
    }

    // If the flow_str contains "(Multiple keys required)" or "(API key)", we handle differently:
    if flow_str.contains("(Multiple keys required)") {
        return handle_multiple_keys_flow(platform, &user.user_id, bot_api);
    }
    if flow_str.contains("(API key)") {
        // Possibly the same approach as multiple keys but with just 1
        // Example: "Please get a chat token from some site..."
        return handle_api_key_flow(flow_str, platform, &user.user_id, bot_api);
    }

    // Otherwise, if the flow is "No prompt needed" or "2FA," etc., you can adapt similarly:
    if flow_str.contains("(2FA)") {
        // ...
        return "(2FA) not yet implemented in TUI example".to_string();
    }
    if flow_str.contains("(No prompt needed)") {
        return "(No prompt needed) Possibly done?".to_string();
    }

    // Fallback:
    flow_str
}

/// If it's a normal OAuth flow requiring a callback:
fn handle_oauth_browser_flow(
    auth_url: String,
    platform: Platform,
    user_id: &Uuid,
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

    // start local callback server
    let port = 9876;
    let (done_rx, shutdown_tx) = match tui_block_on(start_callback_server(port)) {
        Ok(pair) => pair,
        Err(e) => {
            return format!("Error starting callback server => {:?}", e);
        }
    };
    println!("OAuth callback server listening on http://127.0.0.1:{}", port);
    println!("Waiting for the OAuth callback...");

    // wait for callback
    let callback_result = match tui_block_on(async { done_rx.await }) {
        Ok(res) => res,
        Err(e) => {
            let _ = shutdown_tx.send(());
            return format!("Error receiving OAuth code => {:?}", e);
        }
    };
    let _ = shutdown_tx.send(());

    // Now pass that code into complete_auth_flow_for_user
    match tui_block_on(bot_api.complete_auth_flow_for_user(
        platform,
        callback_result.code,
        *user_id,
    )) {
        Ok(cred) => {
            format!(
                "Success! Stored credentials => platform={:?}, user_id={}, is_bot={}",
                cred.platform, cred.user_id, cred.is_bot
            )
        }
        Err(e) => format!("Error completing auth => {:?}", e),
    }
}

/// If it's "Multiple keys required", e.g. Discord or VRChat:
fn handle_multiple_keys_flow(
    platform: Platform,
    user_id: &Uuid,
    bot_api: &Arc<dyn BotApi>
) -> String {
    // In this example, we only know from the authenticator code that Discord will request:
    //   fields: ["bot_token"]
    // but in general, it might be more fields. We'll ask how many keys.
    // We'll just do a short Q&A example here:

    println!("Enter each required field (example: 'bot_token'):");
    let mut keys_map = HashMap::new();

    // For a more robust approach, you'd want the AuthManager to *actually*
    // return the list of fields. But we only got a string. So let's guess:
    println!("(For Discord, you likely only need 'bot_token'. Type the key name now, or leave blank to finish.)");

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

    // Now call complete_auth_flow_for_user_multi
    match tui_block_on(bot_api.complete_auth_flow_for_user_multi(
        platform,
        *user_id,
        keys_map,
    )) {
        Ok(cred) => {
            format!(
                "Success! Stored credentials => platform={:?}, user_id={}, is_bot={}",
                cred.platform, cred.user_id, cred.is_bot
            )
        }
        Err(e) => format!("Error completing multi-key auth => {:?}", e),
    }
}

/// If it's just an "(API key)" single item:
fn handle_api_key_flow(
    prompt_msg: String,
    platform: Platform,
    user_id: &Uuid,
    bot_api: &Arc<dyn BotApi>
) -> String {
    println!("Auth flow said: {}", prompt_msg);

    // user enters the single key
    print!("Paste the API key now:\n> ");
    let _ = stdout().flush();
    let mut key_line = String::new();
    let _ = stdin().read_line(&mut key_line);
    let token_str = key_line.trim().to_string();
    if token_str.is_empty() {
        return "No API key entered. Aborting.".to_string();
    }
    let mut map = HashMap::new();
    // We'll guess the key name is "api_key", but it depends on your platform logic
    map.insert("api_key".into(), token_str);

    // call the multi-keys function
    match tui_block_on(bot_api.complete_auth_flow_for_user_multi(
        platform,
        *user_id,
        map,
    )) {
        Ok(cred) => {
            format!(
                "Success! Stored credentials => platform={:?}, user_id={}, is_bot={}",
                cred.platform, cred.user_id, cred.is_bot
            )
        }
        Err(e) => format!("Error completing single-key flow => {:?}", e),
    }
}

// ------------------------------------------------------------------------
// We re-use the same user creation logic as before
// ------------------------------------------------------------------------
async fn find_or_create_user_by_name(
    bot_api: &Arc<dyn BotApi>,
    final_username: &str
) -> Result<User, Error> {
    // 1) see if user with that name already exists:
    let all = bot_api.search_users(final_username).await?;
    if let Some(u) = all.into_iter().find(|usr| {
        usr.global_username.as_deref().map(|s| s.to_lowercase()) == Some(final_username.to_lowercase())
    }) {
        Ok(u)
    } else {
        // create
        let new_uuid = Uuid::new_v4();
        bot_api.create_user(new_uuid, final_username).await?;
        let user_opt = bot_api.get_user(new_uuid).await?;
        let user = user_opt.ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;
        Ok(user)
    }
}

// ------------------------------------------------------------------------
// "account remove <platform> <usernameOrUUID>"
// ------------------------------------------------------------------------
fn account_remove(platform: Platform, user_id_str: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // interpret user_id_str as either a UUID or a username
    let user_uuid = match Uuid::parse_str(user_id_str) {
        Ok(u) => u,
        Err(_) => {
            // try to find user
            match tui_block_on(bot_api.find_user_by_name(user_id_str)) {
                Ok(u) => u.user_id,
                Err(e) => {
                    return format!("No user found with name '{}': {:?}", user_id_str, e);
                }
            }
        }
    };

    match tui_block_on(bot_api.revoke_credentials(platform.clone(), user_uuid.to_string())) {
        Ok(_) => format!(
            "Removed credentials for platform={:?}, user_id={}",
            platform, user_uuid
        ),
        Err(e) => format!("Error removing => {:?}", e),
    }
}

// ------------------------------------------------------------------------
// "account list" => show known credentials
// ------------------------------------------------------------------------
fn account_list(maybe_platform: Option<Platform>, bot_api: &Arc<dyn BotApi>) -> String {
    let list_result = tui_block_on(bot_api.list_credentials(maybe_platform));
    match list_result {
        Ok(list) => {
            if list.is_empty() {
                "No stored platform credentials.\n".to_string()
            } else {
                let mut out = String::new();
                out.push_str("Stored platform credentials:\n");
                for c in list {
                    let username_or_id = match tui_block_on(bot_api.get_user(c.user_id)) {
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

// ------------------------------------------------------------------------
// "account show <platform> <usernameOrUUID>"
// ------------------------------------------------------------------------
fn account_show(platform: Platform, user_id_str: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let user_uuid = match Uuid::parse_str(user_id_str) {
        Ok(u) => u,
        Err(_) => {
            // try find user
            match tui_block_on(bot_api.find_user_by_name(user_id_str)) {
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

    let all = match tui_block_on(bot_api.list_credentials(Some(platform.clone()))) {
        Ok(list) => list,
        Err(e) => return format!("Error => {:?}", e),
    };

    let maybe_cred = all.into_iter().find(|c| c.user_id == user_uuid);
    match maybe_cred {
        Some(c) => {
            let mut out = String::new();
            out.push_str(&format!("platform={:?}\n", c.platform));
            out.push_str(&format!("user_id={}\n", c.user_id));
            out.push_str(&format!("credential_type={:?}\n", c.credential_type));
            out.push_str(&format!("is_bot={}\n", c.is_bot));
            out.push_str(&format!("primary_token='{}'\n", c.primary_token));
            let refresh_str = match &c.refresh_token {
                Some(rt) => rt,
                None => "(none)",
            };
            out.push_str(&format!("refresh_token='{}'\n", refresh_str));
            out.push_str(&format!("additional_data={:?}\n", c.additional_data));
            out.push_str(&format!("expires_at={:?}\n", c.expires_at));
            out.push_str(&format!("created_at={}\nupdated_at={}\n", c.created_at, c.updated_at));
            out
        }
        None => format!(
            "No credentials found for platform={:?}, user_id='{}'",
            platform, user_uuid
        ),
    }
}