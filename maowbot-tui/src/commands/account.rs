use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::collections::HashMap;
use std::str::FromStr;
use open;
use uuid::Uuid;
use chrono::Utc;

use maowbot_core::models::{Platform, PlatformCredential};
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::Error;

/// Handle "account <add|remove|list|show|refresh>" commands asynchronously.
pub async fn handle_account_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: account <add|remove|list|show|refresh> [platform] [username|UUID]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 3 {
                return "Usage: account add <platform> <typed_global_username>".to_string();
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
        "refresh" => {
            if args.len() < 3 {
                return "Usage: account refresh <platform> <usernameOrUUID>".to_string();
            }
            let platform_str = args[1];
            let user_id_str  = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_refresh(p, user_id_str, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        _ => "Usage: account <add|remove|list|show|refresh> [platform] [username|UUID]".to_string(),
    }
}

/// Main “add” flow for user credentials on a given platform.
async fn account_add_flow(platform: Platform, typed_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    println!("Is this a bot account? (y/n):");
    print!("> ");
    let _ = stdout().flush();
    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    let final_username: String;

    if is_bot {
        println!("Use '{}' for the user’s global_username? (y/n):", typed_name);
        print!("> ");
        let _ = stdout().flush();
        let mut line2 = String::new();
        let _ = stdin().read_line(&mut line2);
        if line2.trim().eq_ignore_ascii_case("y") {
            final_username = typed_name.to_string();
        } else {
            println!("Enter a different global username:");
            print!("> ");
            let _ = stdout().flush();
            let mut alt = String::new();
            let _ = stdin().read_line(&mut alt);
            let alt = alt.trim();
            if alt.is_empty() {
                final_username = typed_name.to_string();
            } else {
                final_username = alt.to_string();
            }
        }
    } else {
        final_username = typed_name.to_string();
    }

    // Step A: find or create user
    let user = match find_or_create_user_by_name(bot_api, &final_username).await {
        Ok(u) => u,
        Err(e) => return format!("Error finding/creating user '{}': {:?}", final_username, e),
    };
    println!(
        "\nWill store new credentials for user_id={}, global_username='{}'",
        user.user_id,
        user.global_username.as_deref().unwrap_or("(none)")
    );

    // Step B: Begin the auth flow
    let main_result = do_oauth_like_flow(platform.clone(), user.user_id, is_bot, bot_api).await;
    if let Err(e) = main_result {
        return format!("Error creating credential for {:?} => {:?}", platform, e);
    }

    // If "twitch" + non-bot => also create twitch-irc and eventsub
    if platform == Platform::Twitch && !is_bot {
        println!("\nBecause this is a non-bot Twitch account, also create matching:\n - twitch-irc\n - twitch-eventsub\n");

        // twitch-irc
        match do_oauth_like_flow(Platform::TwitchIRC, user.user_id, false, bot_api).await {
            Ok(_) => println!("Successfully created twitch-irc credentials.\n"),
            Err(e) => println!("(Warning) Could not create twitch-irc => {:?}", e),
        }

        // re-use helix tokens for eventsub
        match reuse_twitch_helix_for_eventsub(user.user_id, bot_api).await {
            Ok(_) => println!("Successfully created twitch-eventsub credentials.\n"),
            Err(e) => println!("(Warning) Could not create twitch-eventsub => {:?}", e),
        }
    }

    format!("Success! Created credential(s) for user_id={}", user.user_id)
}

/// Standard OAuth-like path for the named platform
async fn do_oauth_like_flow(
    platform: Platform,
    user_id: Uuid,
    is_bot: bool,
    bot_api: &Arc<dyn BotApi>
) -> Result<(), Error> {
    let flow_str = bot_api.begin_auth_flow(platform.clone(), is_bot).await?;
    if flow_str.starts_with("http://") || flow_str.starts_with("https://") {
        println!("Open this URL to authenticate:\n  {}", flow_str);
        println!("Open in browser now? (y/n):");
        print!("> ");
        let _ = stdout().flush();
        let mut line3 = String::new();
        let _ = stdin().read_line(&mut line3);
        if line3.trim().eq_ignore_ascii_case("y") {
            let _ = open::that(&flow_str);
        }

        let (done_rx, shutdown_tx) =
            maowbot_core::auth::callback_server::start_callback_server(9876).await?;
        println!("Waiting for the OAuth callback on http://127.0.0.1:9876 ...");

        let callback_result = match done_rx.await {
            Ok(r) => r,
            Err(e) => {
                let _ = shutdown_tx.send(());
                return Err(Error::Auth(format!("Error receiving OAuth code => {e}")));
            }
        };
        let _ = shutdown_tx.send(());

        bot_api
            .complete_auth_flow_for_user(platform, callback_result.code, user_id)
            .await?;
        Ok(())
    } else if flow_str.contains("(Multiple keys required)") {
        // Discord or VRChat
        if platform == Platform::Discord {
            let keys_map = prompt_discord_bot_token_and_fetch().await?;
            bot_api
                .complete_auth_flow_for_user_multi(platform, user_id, keys_map)
                .await?;
        } else {
            let keys_map = prompt_for_multiple_keys()?;
            bot_api
                .complete_auth_flow_for_user_multi(platform, user_id, keys_map)
                .await?;
        }
        Ok(())
    } else if flow_str.contains("(API key)") {
        println!("Auth flow said: {}", flow_str);
        print!("Paste the API key now:\n> ");
        let _ = stdout().flush();
        let mut key_line = String::new();
        let _ = stdin().read_line(&mut key_line);
        let token_str = key_line.trim().to_string();
        if token_str.is_empty() {
            return Err(Error::Auth("No API key entered.".into()));
        }
        let mut m = HashMap::new();
        m.insert("api_key".into(), token_str);
        bot_api
            .complete_auth_flow_for_user_multi(platform, user_id, m)
            .await?;
        Ok(())
    } else if flow_str.contains("(2FA)") {
        Err(Error::Auth("2FA-based login not implemented in TUI".into()))
    } else if flow_str.contains("(No prompt needed)") {
        println!("No prompt needed, possibly auto-completed.");
        Ok(())
    } else {
        Err(Error::Auth(format!("Unexpected flow prompt => {flow_str}")))
    }
}

/// Re‐use Helix credential to create an eventsub credential for the same user.
pub async fn reuse_twitch_helix_for_eventsub(
    user_id: Uuid,
    bot_api: &Arc<dyn BotApi>,
) -> Result<(), Error> {
    // 1) find the Helix credential for this user
    let all_twitch_creds = bot_api.list_credentials(Some(Platform::Twitch)).await?;
    let helix_cred_opt = all_twitch_creds.into_iter().find(|c| c.user_id == user_id);
    let helix_cred = match helix_cred_opt {
        Some(c) => c,
        None => {
            return Err(Error::Auth(
                "Cannot create eventsub credential: no Twitch Helix cred found.".to_string()
            ));
        }
    };

    // 2) clone it as a new credential for eventsub
    let mut new_cred = helix_cred.clone();
    new_cred.platform = Platform::TwitchEventSub;
    new_cred.credential_id = Uuid::new_v4();
    new_cred.created_at = chrono::Utc::now();
    new_cred.updated_at = chrono::Utc::now();

    // Instead of overwriting the additional_data, let's merge in a "note"
    let merged_data = if let Some(old_data) = &new_cred.additional_data {
        // Convert old_data to a mutable map if possible
        if let Some(mut map) = old_data.as_object().cloned() {
            map.insert("note".to_string(), serde_json::Value::String("EventSub re-uses Helix".into()));
            serde_json::Value::Object(map)
        } else {
            // If not an object, just store a new one
            serde_json::json!({ "note":"EventSub re-uses Helix" })
        }
    } else {
        // If no old data, create a fresh object
        serde_json::json!({ "note":"EventSub re-uses Helix" })
    };
    new_cred.additional_data = Some(merged_data);

    // 3) store it
    bot_api.store_credential(new_cred).await?;
    Ok(())
}

/// For Discord: prompt for a “bot_token,” fetch /users/@me, optionally override
async fn prompt_discord_bot_token_and_fetch() -> Result<HashMap<String, String>, Error> {
    println!("\nDiscord flow => we’ll ask for your Bot token, then fetch /users/@me.\n");

    print!("Paste your Discord Bot Token: ");
    let _ = stdout().flush();
    let mut token_line = String::new();
    stdin().read_line(&mut token_line).ok();
    let bot_token = token_line.trim();
    if bot_token.is_empty() {
        return Err(Error::Auth("Discord: no bot token.".into()));
    }

    let (fetched_id, fetched_name) = match fetch_discord_bot_info_once(bot_token).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("(Warning) Could not fetch from Discord => {e}");
            ("".to_string(), "".to_string())
        }
    };

    let final_id = if fetched_id.is_empty() {
        println!("Enter your Discord Bot User ID:");
        print!("> ");
        let _ = stdout().flush();
        let mut tmp = String::new();
        stdin().read_line(&mut tmp).ok();
        tmp.trim().to_string()
    } else {
        println!("Fetched ID='{}'. Press Enter to keep, or type override:", fetched_id);
        print!("> ");
        let _ = stdout().flush();
        let mut tmp = String::new();
        stdin().read_line(&mut tmp).ok();
        let override_id = tmp.trim();
        if override_id.is_empty() { fetched_id } else { override_id.to_string() }
    };
    if final_id.is_empty() {
        return Err(Error::Auth("Discord: No bot_user_id provided/fetched.".into()));
    }

    let final_name = if fetched_name.is_empty() {
        println!("Enter your Discord Bot Username:");
        print!("> ");
        let _ = stdout().flush();
        let mut tmp = String::new();
        stdin().read_line(&mut tmp).ok();
        tmp.trim().to_string()
    } else {
        println!("Fetched username='{}'. Press Enter to keep, or type override:", fetched_name);
        print!("> ");
        let _ = stdout().flush();
        let mut tmp = String::new();
        stdin().read_line(&mut tmp).ok();
        let override_nm = tmp.trim();
        if override_nm.is_empty() { fetched_name } else { override_nm.to_string() }
    };
    if final_name.is_empty() {
        return Err(Error::Auth("Discord: No bot_username provided/fetched.".into()));
    }

    let mut out = HashMap::new();
    out.insert("bot_token".to_string(), bot_token.to_string());
    out.insert("bot_user_id".to_string(), final_id);
    out.insert("bot_username".to_string(), final_name);
    Ok(out)
}

fn prompt_for_multiple_keys() -> Result<HashMap<String, String>, Error> {
    println!("Enter each required field (e.g. 'username', 'password'). Leave key blank to finish.");
    let mut keys_map = HashMap::new();
    loop {
        print!("Key name (empty to finish) > ");
        let _ = stdout().flush();
        let mut keyname = String::new();
        if stdin().read_line(&mut keyname).is_err() {
            return Err(Error::Auth("Error reading key name".into()));
        }
        let keyname = keyname.trim();
        if keyname.is_empty() {
            break;
        }
        print!("Value for '{keyname}'> ");
        let _ = stdout().flush();
        let mut val = String::new();
        if stdin().read_line(&mut val).is_err() {
            return Err(Error::Auth("Error reading key value".into()));
        }
        let val = val.trim().to_string();
        keys_map.insert(keyname.to_string(), val);
    }
    if keys_map.is_empty() {
        return Err(Error::Auth("No keys entered. Aborting.".into()));
    }
    Ok(keys_map)
}

async fn fetch_discord_bot_info_once(bot_token: &str) -> Result<(String, String), Error> {
    use reqwest::Client;
    #[derive(serde::Deserialize)]
    struct DiscordMe {
        id: String,
        username: String,
        discriminator: String,
    }
    let client = Client::new();
    let resp = client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", format!("Bot {bot_token}"))
        .send()
        .await
        .map_err(|e| Error::Auth(format!("Discord /users/@me => {e}")))?;

    if !resp.status().is_success() {
        return Err(Error::Auth(format!(
            "Discord /users/@me returned HTTP {}",
            resp.status()
        )));
    }
    let body = resp
        .json::<DiscordMe>()
        .await
        .map_err(|e| Error::Auth(format!("Discord parse JSON => {e}")))?;
    Ok((body.id, body.username))
}

async fn account_remove(platform: Platform, user_id_str: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let user_uuid = match Uuid::parse_str(user_id_str) {
        Ok(u) => u,
        Err(_) => {
            // find user by name
            match bot_api.find_user_by_name(user_id_str).await {
                Ok(u) => u.user_id,
                Err(e) => return format!("No user found with name '{}': {:?}", user_id_str, e),
            }
        }
    };

    match bot_api
        .revoke_credentials(platform.clone(), user_uuid.to_string())
        .await
    {
        Ok(_) => format!("Removed credentials for platform='{:?}', user_id={}", platform, user_uuid),
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
                        Ok(Some(u)) => {
                            u.global_username.unwrap_or_else(|| c.user_id.to_string())
                        }
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
        format!(
            "No credentials found for platform={:?}, user_id='{}'",
            platform, user_uuid
        )
    }
}

/// **New**: refresh an existing credential
async fn account_refresh(
    platform: Platform,
    user_id_str: &str,
    bot_api: &Arc<dyn BotApi>
) -> String {
    let user_uuid = match Uuid::parse_str(user_id_str) {
        Ok(u) => u,
        Err(_) => {
            // Try by username
            match bot_api.find_user_by_name(user_id_str).await {
                Ok(u) => u.user_id,
                Err(e) => {
                    return format!("No user found with name '{}': {:?}", user_id_str, e);
                }
            }
        }
    };

    match bot_api.refresh_credentials(platform.clone(), user_uuid.to_string()).await {
        Ok(new_cred) => {
            format!(
                "Successfully refreshed credential for platform={:?}, user_id={}, new expires_at={:?}",
                new_cred.platform, new_cred.user_id, new_cred.expires_at
            )
        }
        Err(e) => format!("Error refreshing => {:?}", e),
    }
}

/// Let the user type a name or reuse existing. If none found, create a new user row.
async fn find_or_create_user_by_name(
    bot_api: &Arc<dyn BotApi>,
    final_username: &str
) -> Result<maowbot_core::models::User, Error> {
    let all = bot_api.search_users(final_username).await?;
    if let Some(u) = all.into_iter().find(|usr| {
        usr.global_username
            .as_deref()
            .map(|s| s.to_lowercase())
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