use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::collections::HashMap;
use std::str::FromStr;

use chrono::Utc;
use open;
use uuid::Uuid;

use maowbot_common::models::auth::Platform;
use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::models::redeem::Redeem;
use maowbot_common::models::user::User;
use maowbot_common::traits::api::BotApi;
use maowbot_core::Error;

/// Handle "account <add|remove|list|show|refresh|type>" commands asynchronously.
pub async fn handle_account_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: account <add|remove|list|show|refresh|type> [platform] [usernameOrUUID]".to_string();
    }

    match args[0] {
        "add" => {
            // account add <platform> <typed_global_username>
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
            // account remove <platform> <usernameOrUUID>
            if args.len() < 3 {
                return "Usage: account remove <platform> <usernameOrUUID>".to_string();
            }
            let platform_str = args[1];
            let user_str     = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_remove(p, user_str, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "list" => {
            // account list [platform?]
            let maybe_platform = if args.len() > 1 {
                // e.g. "account list twitch-irc"
                Platform::from_str(args[1]).ok()
            } else {
                None
            };
            account_list(maybe_platform, bot_api).await
        }
        "show" => {
            // account show <platform> <usernameOrUUID>
            if args.len() < 3 {
                return "Usage: account show <platform> <usernameOrUUID>".to_string();
            }
            let platform_str = args[1];
            let user_str     = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_show(p, user_str, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "refresh" => {
            // account refresh <platform> <usernameOrUUID>
            if args.len() < 3 {
                return "Usage: account refresh <platform> <usernameOrUUID>".to_string();
            }
            let platform_str = args[1];
            let user_str     = args[2];
            match Platform::from_str(platform_str) {
                Ok(p) => account_refresh(p, user_str, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        "type" => {
            // account type <platform> <usernameOrUUID> <bot|broadcaster|teammate>
            if args.len() < 4 {
                return "Usage: account type <platform> <usernameOrUUID> <bot|broadcaster|teammate>".to_string();
            }
            let platform_str = args[1];
            let user_str     = args[2];
            let role_flag    = args[3];
            match Platform::from_str(platform_str) {
                Ok(p) => set_account_type(bot_api, p, user_str, role_flag).await,
                Err(_) => format!("Unknown platform '{}'", platform_str),
            }
        }
        _ => "Usage: account <add|remove|list|show|refresh|type> [platform] [usernameOrUUID]".to_string(),
    }
}

/// Main “add” flow for user credentials on a given platform.
async fn account_add_flow(platform: Platform, typed_name: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // Step 0: Check if there's already a broadcaster for this platform
    let all_creds = match bot_api.list_credentials(Some(platform.clone())).await {
        Ok(list) => list,
        Err(e) => return format!("Error listing existing credentials => {e}"),
    };
    let has_broadcaster = all_creds.iter().any(|c| c.is_broadcaster);

    let mut is_broadcaster = false;
    if !has_broadcaster {
        println!("Is this your broadcaster account [Y/n]");
        print!("> ");
        let _ = stdout().flush();
        let mut line = String::new();
        let _ = stdin().read_line(&mut line);
        let trimmed = line.trim().to_lowercase();
        if trimmed.is_empty() || trimmed == "y" || trimmed == "yes" {
            is_broadcaster = true;
        }
    }

    let mut is_teammate = false;
    if !is_broadcaster {
        println!("Is this a teammate account [y/N]");
        print!("> ");
        let _ = stdout().flush();
        let mut line2 = String::new();
        let _ = stdin().read_line(&mut line2);
        let trimmed2 = line2.trim().to_lowercase();
        if trimmed2 == "y" || trimmed2 == "yes" {
            is_teammate = true;
        }
    }

    let mut is_bot = false;
    if !is_broadcaster && !is_teammate {
        println!("Is this a bot account [Y/n]");
        print!("> ");
        let _ = stdout().flush();
        let mut line3 = String::new();
        let _ = stdin().read_line(&mut line3);
        let trimmed3 = line3.trim().to_lowercase();
        if trimmed3.is_empty() || trimmed3 == "y" || trimmed3 == "yes" {
            is_bot = true;
        }
    }

    // Attempt to finalize a global_username for this user:
    let final_username: String;
    if is_bot {
        println!("Use '{}' for the user’s global_username? (y/n):", typed_name);
        print!("> ");
        let _ = stdout().flush();
        let mut line4 = String::new();
        let _ = stdin().read_line(&mut line4);
        if line4.trim().eq_ignore_ascii_case("y") {
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

    // Step A: find or create the user
    let user = match find_or_create_user_by_name(bot_api, &final_username).await {
        Ok(u) => u,
        Err(e) => return format!("Error finding/creating user '{}': {:?}", final_username, e),
    };
    println!(
        "\nWill store new credentials for user_id={}, global_username='{}'",
        user.user_id,
        user.global_username.as_deref().unwrap_or("(none)")
    );

    // Step B) Do the actual flows
    let cred_result = if platform == Platform::VRChat {
        vrchat_add_flow(platform.clone(), user.user_id, bot_api).await
    } else if platform == Platform::Discord && is_bot {
        discord_bot_add_flow(platform.clone(), user.user_id, bot_api).await
    } else {
        // Standard OAuth
        do_oauth_like_flow_and_return_cred(platform.clone(), user.user_id, is_bot, bot_api).await
    };

    let mut new_credential = match cred_result {
        Ok(c) => c,
        Err(e) => return format!("Error creating credential for {platform:?} => {e}"),
    };

    // Overwrite the newly stored credential's flags
    new_credential.is_broadcaster = is_broadcaster;
    new_credential.is_teammate    = is_teammate;
    new_credential.is_bot         = is_bot;

    // The .store_credential call updates the DB row with these new flags:
    if let Err(e) = bot_api.store_credential(new_credential.clone()).await {
        return format!("Error updating final credential flags => {e}");
    }

    // === NEW: If we just added a Discord account, also upsert it into `discord_accounts`. ===
    if platform == Platform::Discord {
        let disc_id = new_credential.platform_id.clone();  // e.g. the real Discord user/bot ID
        let upsert_result = bot_api
            .upsert_discord_account(typed_name, Some(new_credential.credential_id), disc_id.as_deref())
            .await;
        if let Err(e) = upsert_result {
            println!("(Warning) Could not upsert discord_accounts => {e}");
        }
    }

    // Additional logic for Twitch -> also create twitch-irc and eventsub if not a bot, etc.
    if platform == Platform::Twitch && !is_bot {
        println!("\nBecause this is a non-bot Twitch account, also create matching:\n - twitch-irc\n - twitch-eventsub\n");

        // 1) Make twitch-irc
        let irc_res = do_oauth_like_flow_and_return_cred(Platform::TwitchIRC, user.user_id, false, bot_api).await;
        let mut maybe_irc_cred: Option<PlatformCredential> = None;
        if let Ok(mut irc_cred) = irc_res {
            irc_cred.is_broadcaster = is_broadcaster;
            irc_cred.is_teammate    = is_teammate;
            irc_cred.is_bot         = false;
            if let Err(e) = bot_api.store_credential(irc_cred.clone()).await {
                println!("(Warning) Could not store updated Twitch-IRC flags => {e}");
            } else {
                println!("Created twitch-irc credentials.\n");
                maybe_irc_cred = Some(irc_cred);
            }
        } else if let Err(e) = irc_res {
            println!("(Warning) Could not create twitch-irc => {e}");
        }

        // 2) Reuse Helix tokens for eventsub
        match reuse_twitch_helix_for_eventsub(user.user_id, bot_api).await {
            Ok(_) => {
                // Re-apply flags
                if let Ok(mut ev_sub_cred) = find_credential_for_user(bot_api, user.user_id, Platform::TwitchEventSub).await {
                    ev_sub_cred.is_broadcaster = is_broadcaster;
                    ev_sub_cred.is_teammate    = is_teammate;
                    ev_sub_cred.is_bot         = false;
                    if let Err(e) = bot_api.store_credential(ev_sub_cred).await {
                        println!("(Warning) Could not store updated eventsub flags => {e}");
                    } else {
                        println!("Created twitch-eventsub credentials.\n");
                    }
                }
            }
            Err(e) => {
                println!("(Warning) Could not create twitch-eventsub => {e}");
            }
        }

        // If user is the broadcaster, optionally set existing redeems to use Helix
        if is_broadcaster {
            let helix_cred_id = new_credential.credential_id;
            if let Err(e) = set_existing_redeems_active_cred(bot_api, helix_cred_id).await {
                println!("(Warning) Could not set redeem.active_credential_id => {e}");
            }
        }
    }

    format!("Success! Created credential(s) for user_id={}", user.user_id)
}

/// Utility that updates any existing `twitch-eventsub` Redeems to set `.active_credential_id`.
async fn set_existing_redeems_active_cred(
    bot_api: &Arc<dyn BotApi>,
    twitch_helix_credential_id: Uuid,
) -> Result<(), Error> {
    let redeems = bot_api.list_redeems("twitch-eventsub").await?;
    for mut r in redeems {
        if r.active_credential_id.is_none() {
            r.active_credential_id = Some(twitch_helix_credential_id);
            bot_api.update_redeem(&r).await?;
        }
    }
    Ok(())
}

/// Specialized VRChat flow
async fn vrchat_add_flow(
    platform: Platform,
    user_id: Uuid,
    bot_api: &Arc<dyn BotApi>
) -> Result<PlatformCredential, Error> {
    let initial_prompt = bot_api.begin_auth_flow(platform.clone(), false).await?;
    if !initial_prompt.contains("MultipleKeys") {
        return Err(Error::Auth(format!("Unexpected VRChat prompt => {initial_prompt}")));
    }

    print!("Enter your VRChat username: ");
    let _ = stdout().flush();
    let mut lineu = String::new();
    stdin().read_line(&mut lineu).ok();

    print!("Enter your VRChat password: ");
    let _ = stdout().flush();
    let mut linep = String::new();
    stdin().read_line(&mut linep).ok();

    let mut keys_map = HashMap::new();
    keys_map.insert("username".to_string(), lineu.trim().to_string());
    keys_map.insert("password".to_string(), linep.trim().to_string());

    let first_res = bot_api
        .complete_auth_flow_for_user_multi(platform.clone(), user_id, keys_map)
        .await;

    match first_res {
        Ok(first_cred) => Ok(first_cred),
        Err(err) => {
            let msg = format!("{err}");
            if msg.contains("__2FA_PROMPT__") {
                println!("VRChat login requires 2FA code!");
                print!("Enter your 2FA code: ");
                let _ = stdout().flush();
                let mut linec = String::new();
                stdin().read_line(&mut linec).ok();
                let code = linec.trim().to_string();

                let second_result = bot_api
                    .complete_auth_flow_for_user_2fa(platform.clone(), code, user_id)
                    .await;
                match second_result {
                    Ok(cred2) => {
                        println!("VRChat 2FA success! Credentials stored for user_id={user_id}");
                        Ok(cred2)
                    }
                    Err(e2) => Err(Error::Auth(format!("VRChat 2FA error => {e2}"))),
                }
            } else {
                Err(Error::Auth(format!("VRChat login failed => {msg}")))
            }
        }
    }
}

/// Specialized Discord flow for bot accounts
async fn discord_bot_add_flow(
    platform: Platform,
    user_id: Uuid,
    bot_api: &Arc<dyn BotApi>
) -> Result<PlatformCredential, Error> {
    let flow_str = bot_api.begin_auth_flow(platform.clone(), true).await?;
    if !flow_str.contains("MultipleKeys") {
        return Err(Error::Auth(format!("Unexpected Discord bot prompt => {flow_str}")));
    }

    let map = prompt_discord_bot_token_and_fetch().await?;
    let final_cred = bot_api
        .complete_auth_flow_for_user_multi(platform, user_id, map)
        .await?;
    Ok(final_cred)
}

/// Standard OAuth-like path
async fn do_oauth_like_flow_and_return_cred(
    platform: Platform,
    user_id: Uuid,
    is_bot: bool,
    bot_api: &Arc<dyn BotApi>
) -> Result<PlatformCredential, Error> {
    let flow_str = bot_api.begin_auth_flow(platform.clone(), is_bot).await?;
    if flow_str.starts_with("http://") || flow_str.starts_with("https://") {
        println!("Open this URL to authenticate:\n  {flow_str}");
        if is_bot {
            println!(
                "(Bot account) Attempting incognito. If it fails, open manually.\n\
                 Or sign out of your main account first.\n"
            );
            let _ = try_open_incognito(&flow_str);
        } else {
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

        let new_cred = bot_api
            .complete_auth_flow_for_user(platform, callback_result.code, user_id)
            .await?;
        Ok(new_cred)
    }
    else if flow_str.contains("(MultipleKeys)") {
        println!("Auth flow said: {flow_str}");
        let keys_map = prompt_for_multiple_keys()?;
        let cred = bot_api
            .complete_auth_flow_for_user_multi(platform, user_id, keys_map)
            .await?;
        Ok(cred)
    }
    else if flow_str.contains("(API key)") {
        println!("Auth flow said: {flow_str}");
        print!("Paste the API key now:\n> ");
        let _ = stdout().flush();
        let mut key_line = String::new();
        stdin().read_line(&mut key_line).ok();
        let token_str = key_line.trim().to_string();
        if token_str.is_empty() {
            return Err(Error::Auth("No API key entered.".into()));
        }
        let mut m = HashMap::new();
        m.insert("api_key".into(), token_str);
        let cred = bot_api
            .complete_auth_flow_for_user_multi(platform, user_id, m)
            .await?;
        Ok(cred)
    }
    else if flow_str.contains("(2FA)") {
        Err(Error::Auth(
            "2FA-based login not implemented in TUI for this platform.".into(),
        ))
    }
    else {
        Err(Error::Auth(format!("Unexpected flow prompt => {flow_str}")))
    }
}

/// Reuse Helix credential to create an EventSub credential
pub async fn reuse_twitch_helix_for_eventsub(
    user_id: Uuid,
    bot_api: &Arc<dyn BotApi>,
) -> Result<(), Error> {
    let all_twitch_creds = bot_api.list_credentials(Some(Platform::Twitch)).await?;
    let helix_cred_opt = all_twitch_creds.into_iter().find(|c| c.user_id == user_id);
    let helix_cred = match helix_cred_opt {
        Some(c) => c,
        None => {
            return Err(Error::Auth("No Twitch Helix cred found for that user.".to_string()));
        }
    };

    let mut new_cred = helix_cred.clone();
    new_cred.platform = Platform::TwitchEventSub;
    new_cred.credential_id = Uuid::new_v4();
    new_cred.created_at = Utc::now();
    new_cred.updated_at = Utc::now();

    let merged_data = if let Some(old_data) = &new_cred.additional_data {
        if let Some(mut map) = old_data.as_object().cloned() {
            map.insert("note".to_string(), serde_json::Value::String("EventSub re-uses Helix".into()));
            serde_json::Value::Object(map)
        } else {
            serde_json::json!({ "note":"EventSub re-uses Helix" })
        }
    } else {
        serde_json::json!({ "note":"EventSub re-uses Helix" })
    };
    new_cred.additional_data = Some(merged_data);

    bot_api.store_credential(new_cred).await?;
    Ok(())
}

/// Attempt to open a URL in incognito mode if supported
fn try_open_incognito(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-a")
            .arg("Google Chrome")
            .arg("--args")
            .arg("--incognito")
            .arg(url)
            .spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let url_escaped = url.replace("&", "^&");
        std::process::Command::new("cmd")
            .args(&["/C", "start", "chrome", "--incognito", &url_escaped])
            .spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let status_chrome = std::process::Command::new("google-chrome")
            .arg("--incognito")
            .arg(url)
            .spawn();
        match status_chrome {
            Ok(_) => Ok(()),
            Err(_) => {
                std::process::Command::new("chromium")
                    .arg("--incognito")
                    .arg(url)
                    .spawn()?;
                Ok(())
            }
        }?;
        return Ok(());
    }

    // fallback:
    Err("Incognito opening not implemented for this platform.".into())
}

fn prompt_for_multiple_keys() -> Result<HashMap<String, String>, Error> {
    println!("Enter each required field. Leave key blank to finish.");
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

/// Prompt user for a Discord Bot Token and attempt to fetch /users/@me, plus the "app_id".
async fn prompt_discord_bot_token_and_fetch() -> Result<HashMap<String, String>, Error> {
    use reqwest::Client;
    #[derive(serde::Deserialize)]
    struct DiscordMe {
        id: String,
        username: String,
        discriminator: String,
    }

    println!("\nDiscord flow => we’ll ask for your Bot token, fetch /users/@me, and then your Application ID.\n");
    print!("Paste your Discord Bot Token: ");
    let _ = stdout().flush();
    let mut token_line = String::new();
    stdin().read_line(&mut token_line).ok();
    let bot_token = token_line.trim();
    if bot_token.is_empty() {
        return Err(Error::Auth("Discord: no bot token.".into()));
    }

    let client = Client::new();
    let resp = match client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", format!("Bot {bot_token}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("(Warning) Could not call /users/@me => {e}");
            return read_discord_bot_ids_manually(bot_token).await;
        }
    };

    if !resp.status().is_success() {
        eprintln!("(Warning) /users/@me returned HTTP {}", resp.status());
        return read_discord_bot_ids_manually(bot_token).await;
    }

    let body: DiscordMe = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("(Warning) Could not parse Discord /users/@me => {e}");
            return read_discord_bot_ids_manually(bot_token).await;
        }
    };

    println!("Fetched ID='{}'. Press Enter to keep, or type override:", body.id);
    print!("> ");
    let _ = stdout().flush();
    let mut tmp_id = String::new();
    stdin().read_line(&mut tmp_id).ok();
    let final_id = {
        let trimmed = tmp_id.trim();
        if trimmed.is_empty() {
            &body.id
        } else {
            trimmed
        }
    };

    println!("Fetched username='{}'. Press Enter to keep, or type override:", body.username);
    print!("> ");
    let _ = stdout().flush();
    let mut tmp_nm = String::new();
    stdin().read_line(&mut tmp_nm).ok();
    let final_name = {
        let trimmed = tmp_nm.trim();
        if trimmed.is_empty() {
            &body.username
        } else {
            trimmed
        }
    };

    println!("Enter your Discord Application ID (App ID):");
    print!("> ");
    let _ = stdout().flush();
    let mut line_app = String::new();
    stdin().read_line(&mut line_app).ok();
    let final_app_id = line_app.trim().to_string();

    let mut map = HashMap::new();
    map.insert("bot_token".to_string(), bot_token.to_string());
    map.insert("bot_user_id".to_string(), final_id.to_string());
    map.insert("bot_username".to_string(), final_name.to_string());
    map.insert("bot_app_id".to_string(), final_app_id);
    Ok(map)
}

async fn read_discord_bot_ids_manually(bot_token: &str) -> Result<HashMap<String, String>, Error> {
    println!("Enter your Discord Bot User ID:");
    print!("> ");
    let _ = stdout().flush();
    let mut line_id = String::new();
    stdin().read_line(&mut line_id).ok();
    let final_id = line_id.trim().to_string();
    if final_id.is_empty() {
        return Err(Error::Auth("Discord: no bot_user_id provided.".into()));
    }

    println!("Enter your Discord Bot Username:");
    print!("> ");
    let _ = stdout().flush();
    let mut line_un = String::new();
    stdin().read_line(&mut line_un).ok();
    let final_name = line_un.trim().to_string();
    if final_name.is_empty() {
        return Err(Error::Auth("Discord: no bot_username provided.".into()));
    }

    println!("Enter your Discord Application ID (App ID):");
    print!("> ");
    let _ = stdout().flush();
    let mut line_app = String::new();
    stdin().read_line(&mut line_app).ok();
    let final_app_id = line_app.trim().to_string();

    let mut map = HashMap::new();
    map.insert("bot_token".to_string(), bot_token.to_string());
    map.insert("bot_user_id".to_string(), final_id);
    map.insert("bot_username".to_string(), final_name);
    map.insert("bot_app_id".to_string(), final_app_id);
    Ok(map)
}

/// Remove credentials for a user on a given platform
async fn account_remove(platform: Platform, user_id_str: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let user_uuid = match Uuid::parse_str(user_id_str) {
        Ok(u) => u,
        Err(_) => {
            // find user by name
            match bot_api.find_user_by_name(user_id_str).await {
                Ok(u) => u.user_id,
                Err(e) => return format!("No user found with name '{}': {e}", user_id_str),
            }
        }
    };

    match bot_api
        .revoke_credentials(platform.clone(), user_uuid.to_string())
        .await
    {
        Ok(_) => format!("Removed credentials for platform='{platform:?}', user_id={user_uuid}"),
        Err(e) => format!("Error removing => {e}"),
    }
}

/// List all credentials
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
                        " - user='{}' platform={:?} is_bot={} is_teammate={} is_broadcaster={} credential_id={}\n",
                        username_or_id,
                        c.platform,
                        c.is_bot,
                        c.is_teammate,
                        c.is_broadcaster,
                        c.credential_id
                    ));
                }
                out
            }
        }
        Err(e) => format!("Error => {e}"),
    }
}

/// Show detailed info for one user’s credentials on a single platform
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
                        "No credentials found for platform={platform:?}, user='{user_id_str}'"
                    );
                }
            }
        }
    };

    let all = match bot_api.list_credentials(Some(platform.clone())).await {
        Ok(list) => list,
        Err(e) => return format!("Error => {e}"),
    };

    if let Some(c) = all.into_iter().find(|cred| cred.user_id == user_uuid) {
        let mut out = String::new();
        out.push_str(&format!("platform={:?}\n", c.platform));
        out.push_str(&format!("user_id={}\n", c.user_id));
        out.push_str(&format!("credential_type={:?}\n", c.credential_type));
        out.push_str(&format!("is_bot={}\n", c.is_bot));
        out.push_str(&format!("is_teammate={}\n", c.is_teammate));
        out.push_str(&format!("is_broadcaster={}\n", c.is_broadcaster));
        out.push_str(&format!("primary_token='{}'\n", c.primary_token));
        let refresh_str = c.refresh_token.as_deref().unwrap_or("(none)");
        out.push_str(&format!("refresh_token='{}'\n", refresh_str));
        out.push_str(&format!("additional_data={:?}\n", c.additional_data));
        out.push_str(&format!("expires_at={:?}\n", c.expires_at));
        out.push_str(&format!("created_at={}\nupdated_at={}\n", c.created_at, c.updated_at));
        out
    } else {
        format!(
            "No credentials found for platform={platform:?}, user_id='{user_uuid}'"
        )
    }
}

/// Refresh an existing credential
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
                    return format!("No user found with name '{}': {e}", user_id_str);
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
        Err(e) => format!("Error refreshing => {e}"),
    }
}

/// Let the user type a name or reuse existing. If none found, create a new user row.
async fn find_or_create_user_by_name(
    bot_api: &Arc<dyn BotApi>,
    final_username: &str
) -> Result<User, Error> {
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

/// Helper to locate a single credential for (platform, user_id)
async fn find_credential_for_user(
    bot_api: &Arc<dyn BotApi>,
    user_id: Uuid,
    platform: Platform,
) -> Result<PlatformCredential, Error> {
    let list = bot_api.list_credentials(Some(platform.clone())).await?;
    if let Some(c) = list.into_iter().find(|cred| cred.user_id == user_id) {
        Ok(c)
    } else {
        Err(Error::Platform(format!(
            "No credential found for user_id={user_id} platform={platform:?}"
        )))
    }
}

/// Sets is_bot/is_teammate/is_broadcaster for an existing credential row
async fn set_account_type(
    bot_api: &Arc<dyn BotApi>,
    platform: Platform,
    user_str: &str,
    role_flag: &str
) -> String {
    let user_uuid = match Uuid::parse_str(user_str) {
        Ok(u) => u,
        Err(_) => {
            // try by name
            match bot_api.find_user_by_name(user_str).await {
                Ok(u) => u.user_id,
                Err(_) => return format!("No user found: {user_str}"),
            }
        }
    };

    let all = match bot_api.list_credentials(Some(platform.clone())).await {
        Ok(c) => c,
        Err(e) => return format!("Error listing credentials => {e}"),
    };
    let mut cred_opt = all.into_iter().find(|c| c.user_id == user_uuid);

    let c = match cred_opt.as_mut() {
        Some(xx) => xx,
        None => return format!("No credential found for platform={platform:?}, user_id={user_uuid}"),
    };

    match role_flag.to_lowercase().as_str() {
        "bot" => {
            c.is_bot = true;
            c.is_teammate = false;
            c.is_broadcaster = false;
        }
        "broadcaster" => {
            c.is_bot = false;
            c.is_teammate = false;
            c.is_broadcaster = true;
        }
        "teammate" => {
            c.is_bot = false;
            c.is_teammate = true;
            c.is_broadcaster = false;
        }
        other => {
            return format!(
                "Unrecognized role flag: '{}'; must be one of: bot, broadcaster, teammate",
                other
            );
        }
    }

    if let Err(e) = bot_api.store_credential(c.clone()).await {
        return format!("Error updating credential => {e}");
    }

    format!(
        "Updated credential: is_bot={}, is_teammate={}, is_broadcaster={}",
        c.is_bot, c.is_teammate, c.is_broadcaster
    )
}
