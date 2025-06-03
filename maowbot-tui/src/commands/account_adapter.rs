// Account command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::account::AccountCommands};
use std::io::{Write, stdin, stdout};
use std::collections::HashMap;
use maowbot_proto::maowbot::services::{
    BeginAuthFlowRequest, CompleteAuthFlowRequest, ListCredentialsRequest,
    credential_service_client::CredentialServiceClient,
    complete_auth_flow_request::{self, AuthData}
};
use maowbot_proto::maowbot::common::Platform;

pub async fn handle_account_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: account <add|remove|list|show|refresh|type> [platform] [usernameOrUUID]".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 3 {
                return "Usage: account add <platform> <typed_global_username>".to_string();
            }
            let platform_str = args[1];
            let typed_name = args[2];
            
            // Parse platform
            let platform = match parse_platform(platform_str) {
                Ok(p) => p,
                Err(e) => return format!("Unknown platform '{}': {}", platform_str, e),
            };
            
            // Check if there's already a broadcaster for this platform
            let has_broadcaster = match check_has_broadcaster(client, platform).await {
                Ok(has) => has,
                Err(e) => return format!("Error checking existing credentials: {}", e),
            };
            
            // Ask if broadcaster account
            let mut is_broadcaster = false;
            if !has_broadcaster {
                println!("Is this your broadcaster account [Y/n]");
                print!("> ");
                let _ = stdout().flush();
                let mut line = String::new();
                let _ = stdin().read_line(&mut line);
                let trimmed = line.trim().to_lowercase();
                is_broadcaster = trimmed.is_empty() || trimmed == "y" || trimmed == "yes";
            }
            
            // Ask if teammate account
            let mut is_teammate = false;
            if !is_broadcaster {
                println!("Is this a teammate account [y/N]");
                print!("> ");
                let _ = stdout().flush();
                let mut line2 = String::new();
                let _ = stdin().read_line(&mut line2);
                let trimmed2 = line2.trim().to_lowercase();
                is_teammate = trimmed2 == "y" || trimmed2 == "yes";
            }
            
            // Ask if bot account
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
            
            // Determine final username
            let final_username = if is_bot {
                println!("Use '{}' for the user's global_username? (y/n):", typed_name);
                print!("> ");
                let _ = stdout().flush();
                let mut line4 = String::new();
                let _ = stdin().read_line(&mut line4);
                if line4.trim().eq_ignore_ascii_case("y") {
                    typed_name.to_string()
                } else {
                    println!("Enter a different global username:");
                    print!("> ");
                    let _ = stdout().flush();
                    let mut alt = String::new();
                    let _ = stdin().read_line(&mut alt);
                    let alt = alt.trim();
                    if alt.is_empty() {
                        typed_name.to_string()
                    } else {
                        alt.to_string()
                    }
                }
            } else {
                typed_name.to_string()
            };
            
            // Find or create user
            let user_id = match find_or_create_user(client, &final_username).await {
                Ok(id) => id,
                Err(e) => return format!("Error finding/creating user '{}': {}", final_username, e),
            };
            
            println!("\nWill store new credentials for user_id={}, global_username='{}'", user_id, final_username);
            
            // Do the actual auth flow based on platform
            let result = if platform == Platform::Vrchat {
                vrchat_add_flow(client, platform, user_id).await
            } else if platform == Platform::Discord && is_bot {
                discord_bot_add_flow(client, platform, user_id).await
            } else {
                oauth_add_flow(client, platform, user_id, is_bot).await
            };
            
            match result {
                Ok(credential_id) => {
                    // Update credential flags
                    if let Err(e) = update_credential_flags(client, &credential_id, is_bot, is_broadcaster, is_teammate).await {
                        return format!("Created credential but failed to update flags: {}", e);
                    }
                    
                    // Platform-specific post-processing
                    if platform == Platform::Discord {
                        // TODO: Call upsert_discord_account when available in gRPC API
                    }
                    
                    if platform == Platform::TwitchHelix && !is_bot {
                        println!("\nBecause this is a non-bot Twitch account, also create matching:\n - twitch-irc\n - twitch-eventsub\n");
                        
                        // Create twitch-irc
                        if let Ok(irc_id) = oauth_add_flow(client, Platform::TwitchIrc, user_id, false).await {
                            let _ = update_credential_flags(client, &irc_id, false, is_broadcaster, is_teammate).await;
                            println!("Created twitch-irc credentials.\n");
                        }
                        
                        // Create twitch-eventsub (reusing helix tokens)
                        if let Err(e) = reuse_twitch_helix_for_eventsub(client, user_id).await {
                            println!("(Warning) Could not create twitch-eventsub => {}", e);
                        } else {
                            println!("Created twitch-eventsub credentials.\n");
                        }
                        
                        // If broadcaster, set redeem active_credential_id
                        if is_broadcaster {
                            // TODO: Call set_existing_redeems_active_cred when redeem API is available
                        }
                    }
                    
                    format!("Success! Created credential(s) for user_id={}", user_id)
                }
                Err(e) => format!("Error creating credential for {:?} => {}", platform, e),
            }
        }
        
        "remove" => {
            if args.len() < 3 {
                return "Usage: account remove <platform> <usernameOrUUID>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            
            match AccountCommands::remove_account(client, platform, user_str).await {
                Ok(result) => result.message,
                Err(e) => format!("Error removing account: {}", e),
            }
        }
        
        "list" => {
            let platform = args.get(1).map(|s| *s);
            
            match AccountCommands::list_accounts(client, platform).await {
                Ok(result) => {
                    if result.credentials.is_empty() {
                        "No stored platform credentials.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Stored platform credentials:\n");
                        for cred in result.credentials {
                            out.push_str(&format!(
                                " - user='{}' platform={} is_bot={} credential_id={}\n",
                                cred.username,
                                cred.platform,
                                cred.is_bot,
                                cred.credential_id
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing accounts: {}", e),
            }
        }
        
        "show" => {
            if args.len() < 3 {
                return "Usage: account show <platform> <usernameOrUUID>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            
            match AccountCommands::show_account(client, platform, user_str).await {
                Ok(result) => {
                    if let Some(cred) = result.credential {
                        let mut out = String::new();
                        out.push_str(&format!("platform={}\n", cred.platform));
                        out.push_str(&format!("user_id={}\n", cred.user_id));
                        out.push_str(&format!("is_bot={}\n", cred.is_bot));
                        out.push_str(&format!("is_active={}\n", cred.is_active));
                        out.push_str(&format!("expires_at={:?}\n", cred.expires_at));
                        out.push_str(&format!("created_at={}\n", cred.created_at));
                        out.push_str(&format!("last_refreshed={:?}\n", cred.last_refreshed));
                        out
                    } else {
                        format!("No credentials found for platform={}, user='{}'", platform, user_str)
                    }
                }
                Err(e) => format!("Error showing account: {}", e),
            }
        }
        
        "refresh" => {
            if args.len() < 3 {
                return "Usage: account refresh <platform> <usernameOrUUID>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            
            match AccountCommands::refresh_account(client, platform, user_str).await {
                Ok(result) => result.message,
                Err(e) => format!("Error refreshing account: {}", e),
            }
        }
        
        "type" => {
            if args.len() < 4 {
                return "Usage: account type <platform> <usernameOrUUID> <bot|broadcaster|teammate>".to_string();
            }
            let platform = args[1];
            let user_str = args[2];
            let role_flag = args[3];
            
            let (is_bot, is_broadcaster, is_teammate) = match role_flag.to_lowercase().as_str() {
                "bot" => (true, false, false),
                "broadcaster" => (false, true, false),
                "teammate" => (false, false, true),
                _ => return format!("Unrecognized role flag: '{}'; must be one of: bot, broadcaster, teammate", role_flag),
            };
            
            match AccountCommands::set_account_type(client, platform, user_str, is_bot, is_broadcaster, is_teammate).await {
                Ok(result) => result.message,
                Err(e) => format!("Error setting account type: {}", e),
            }
        }
        
        _ => "Usage: account <add|remove|list|show|refresh|type> [platform] [usernameOrUUID]".to_string(),
    }
}

// Helper functions

async fn check_has_broadcaster(client: &GrpcClient, platform: Platform) -> Result<bool, String> {
    let request = ListCredentialsRequest {
        platforms: vec![platform as i32],
        active_only: false,
        include_expired: true,
        page: None,
    };
    
    let mut cred_client = client.credential.clone();
    let response = cred_client
        .list_credentials(request)
        .await
        .map_err(|e| e.to_string())?;
        
    let has_broadcaster = response.into_inner().credentials.iter().any(|info| {
        info.credential.as_ref().map(|c| c.is_broadcaster).unwrap_or(false)
    });
    
    Ok(has_broadcaster)
}

async fn find_or_create_user(client: &GrpcClient, username: &str) -> Result<String, String> {
    use maowbot_proto::maowbot::services::{SearchUsersRequest, CreateUserRequest};
    
    // Search for existing user
    let mut user_client = client.user.clone();
    let search_request = SearchUsersRequest {
        query: username.to_string(),
        limit: 10,
        offset: 0,
    };
    
    let search_response = user_client
        .search_users(search_request)
        .await
        .map_err(|e| e.to_string())?;
        
    let users = search_response.into_inner().users;
    
    // Look for exact match
    if let Some(user) = users.iter().find(|u| {
        u.global_username.to_lowercase() == username.to_lowercase()
    }) {
        return Ok(user.user_id.clone());
    }
    
    // Create new user
    let create_request = CreateUserRequest {
        user_id: None, // Let server generate
        global_username: username.to_string(),
        is_active: true,
    };
    
    let create_response = user_client
        .create_user(create_request)
        .await
        .map_err(|e| e.to_string())?;
        
    Ok(create_response.into_inner().user.unwrap().user_id)
}

async fn oauth_add_flow(
    client: &GrpcClient,
    platform: Platform,
    user_id: String,
    is_bot: bool,
) -> Result<String, String> {
    // Begin auth flow
    let request = BeginAuthFlowRequest {
        platform: platform as i32,
        is_bot,
        redirect_uri: "http://127.0.0.1:9876".to_string(),
        requested_scopes: vec![],
    };
    
    let mut cred_client = client.credential.clone();
    let begin_response = cred_client
        .begin_auth_flow(request)
        .await
        .map_err(|e| e.to_string())?;
        
    let begin_response = begin_response.into_inner();
    let auth_url = begin_response.auth_url;
    let state = begin_response.state;
    
    // Check if this is a special flow (MultipleKeys, API key, etc)
    if auth_url.contains("(MultipleKeys)") {
        println!("  (MultipleKeys) handle in TUI");
        // For now, return error since we haven't implemented multi-key flow
        return Err("Multi-key authentication not yet implemented in gRPC TUI".to_string());
    }
    
    // Standard OAuth flow
    println!("Open this URL to authenticate:\n  {}", auth_url);
    if is_bot {
        println!("(Bot account) Attempting incognito. If it fails, open manually.");
        println!("Or sign out of your main account first.\n");
        let _ = try_open_incognito(&auth_url);
    } else {
        let _ = open::that(&auth_url);
    }
    
    // Start callback server
    use maowbot_core::auth::callback_server;
    let (done_rx, shutdown_tx) = callback_server::start_callback_server(9876).await
        .map_err(|e| format!("Failed to start callback server: {}", e))?;
        
    println!("OAuth flow initiated. Please complete authentication in your browser.");
    
    // Wait for callback
    let callback_result = done_rx.await
        .map_err(|e| {
            let _ = shutdown_tx.send(());
            format!("Error receiving OAuth code: {}", e)
        })?;
    let _ = shutdown_tx.send(());
    
    // Complete auth flow
    let complete_request = CompleteAuthFlowRequest {
        platform: platform as i32,
        state: state,
        auth_data: Some(AuthData::OauthCode(complete_auth_flow_request::OauthCode {
            code: callback_result.code,
            user_id: user_id,
        })),
    };
    
    let complete_response = cred_client
        .complete_auth_flow(complete_request)
        .await
        .map_err(|e| format!("Failed to complete auth flow: {}", e))?;
        
    let credential = complete_response.into_inner().credential
        .ok_or("No credential returned from auth flow")?;
        
    Ok(credential.credential_id)
}

async fn vrchat_add_flow(
    client: &GrpcClient,
    platform: Platform,
    user_id: String,
) -> Result<String, String> {
    // Begin auth flow to check if it's multi-key
    let request = BeginAuthFlowRequest {
        platform: platform as i32,
        is_bot: false,
        redirect_uri: "".to_string(),
        requested_scopes: vec![],
    };
    
    let mut cred_client = client.credential.clone();
    let begin_response = cred_client
        .begin_auth_flow(request)
        .await
        .map_err(|e| e.to_string())?;
        
    let begin_response = begin_response.into_inner();
    
    if !begin_response.auth_url.contains("(MultipleKeys)") {
        return Err("Unexpected VRChat auth flow response".to_string());
    }
    
    // Prompt for username and password
    print!("Enter your VRChat username: ");
    let _ = stdout().flush();
    let mut username_line = String::new();
    stdin().read_line(&mut username_line).map_err(|e| e.to_string())?;
    
    print!("Enter your VRChat password: ");
    let _ = stdout().flush();
    let mut password_line = String::new();
    stdin().read_line(&mut password_line).map_err(|e| e.to_string())?;
    
    let mut keys_map = HashMap::new();
    keys_map.insert("username".to_string(), username_line.trim().to_string());
    keys_map.insert("password".to_string(), password_line.trim().to_string());
    
    // Complete auth flow with credentials
    let complete_request = CompleteAuthFlowRequest {
        platform: platform as i32,
        state: begin_response.state,
        auth_data: Some(AuthData::CredentialsMap(complete_auth_flow_request::CredentialsMap {
            credentials: keys_map,
            user_id: user_id.clone(),
        })),
    };
    
    let complete_response = cred_client
        .complete_auth_flow(complete_request)
        .await;
        
    match complete_response {
        Ok(resp) => {
            let credential = resp.into_inner().credential
                .ok_or("No credential returned from auth flow")?;
            Ok(credential.credential_id)
        }
        Err(status) => {
            let msg = status.message();
            if msg.contains("__2FA_PROMPT__") {
                println!("VRChat login requires 2FA code!");
                print!("Enter your 2FA code: ");
                let _ = stdout().flush();
                let mut code_line = String::new();
                stdin().read_line(&mut code_line).map_err(|e| e.to_string())?;
                let code = code_line.trim().to_string();
                
                // Complete with 2FA
                let twofa_request = CompleteAuthFlowRequest {
                    platform: platform as i32,
                    state: begin_response.state,
                    auth_data: Some(AuthData::TwoFactorCode(complete_auth_flow_request::TwoFactorCode {
                        code,
                        user_id,
                    })),
                };
                
                let twofa_response = cred_client
                    .complete_auth_flow(twofa_request)
                    .await
                    .map_err(|e| format!("VRChat 2FA error: {}", e))?;
                    
                let credential = twofa_response.into_inner().credential
                    .ok_or("No credential returned from 2FA auth flow")?;
                    
                println!("VRChat 2FA success! Credentials stored for user_id={}", credential.user_id);
                Ok(credential.credential_id)
            } else {
                Err(format!("VRChat login failed: {}", msg))
            }
        }
    }
}

async fn discord_bot_add_flow(
    client: &GrpcClient,
    platform: Platform,
    user_id: String,
) -> Result<String, String> {
    // Begin auth flow to check if it's multi-key
    let request = BeginAuthFlowRequest {
        platform: platform as i32,
        is_bot: true,
        redirect_uri: "".to_string(),
        requested_scopes: vec![],
    };
    
    let mut cred_client = client.credential.clone();
    let begin_response = cred_client
        .begin_auth_flow(request)
        .await
        .map_err(|e| e.to_string())?;
        
    let begin_response = begin_response.into_inner();
    
    if !begin_response.auth_url.contains("(MultipleKeys)") {
        return Err("Unexpected Discord bot auth flow response".to_string());
    }
    
    // Prompt for bot token and fetch bot info
    let keys_map = prompt_discord_bot_token_and_fetch().await?;
    
    // Complete auth flow with credentials
    let complete_request = CompleteAuthFlowRequest {
        platform: platform as i32,
        state: begin_response.state,
        auth_data: Some(AuthData::CredentialsMap(complete_auth_flow_request::CredentialsMap {
            credentials: keys_map,
            user_id,
        })),
    };
    
    let complete_response = cred_client
        .complete_auth_flow(complete_request)
        .await
        .map_err(|e| format!("Failed to complete Discord bot auth flow: {}", e))?;
        
    let credential = complete_response.into_inner().credential
        .ok_or("No credential returned from auth flow")?;
        
    Ok(credential.credential_id)
}

async fn prompt_discord_bot_token_and_fetch() -> Result<HashMap<String, String>, String> {
    use reqwest::Client;
    #[derive(serde::Deserialize)]
    struct DiscordMe {
        id: String,
        username: String,
        discriminator: String,
    }

    println!("\nDiscord flow => we'll ask for your Bot token, fetch /users/@me, and then your Application ID.\n");
    print!("Paste your Discord Bot Token: ");
    let _ = stdout().flush();
    let mut token_line = String::new();
    stdin().read_line(&mut token_line).map_err(|e| e.to_string())?;
    let bot_token = token_line.trim();
    if bot_token.is_empty() {
        return Err("Discord: no bot token.".into());
    }

    let client = Client::new();
    let resp = match client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", format!("Bot {}", bot_token))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("(Warning) Could not call /users/@me => {}", e);
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
            eprintln!("(Warning) Could not parse Discord /users/@me => {}", e);
            return read_discord_bot_ids_manually(bot_token).await;
        }
    };

    println!("Fetched ID='{}'. Press Enter to keep, or type override:", body.id);
    print!("> ");
    let _ = stdout().flush();
    let mut tmp_id = String::new();
    stdin().read_line(&mut tmp_id).map_err(|e| e.to_string())?;
    let final_id = {
        let trimmed = tmp_id.trim();
        if trimmed.is_empty() {
            body.id
        } else {
            trimmed.to_string()
        }
    };

    println!("Fetched username='{}'. Press Enter to keep, or type override:", body.username);
    print!("> ");
    let _ = stdout().flush();
    let mut tmp_nm = String::new();
    stdin().read_line(&mut tmp_nm).map_err(|e| e.to_string())?;
    let final_name = {
        let trimmed = tmp_nm.trim();
        if trimmed.is_empty() {
            body.username
        } else {
            trimmed.to_string()
        }
    };

    println!("Enter your Discord Application ID (App ID):");
    print!("> ");
    let _ = stdout().flush();
    let mut line_app = String::new();
    stdin().read_line(&mut line_app).map_err(|e| e.to_string())?;
    let final_app_id = line_app.trim().to_string();

    let mut map = HashMap::new();
    map.insert("bot_token".to_string(), bot_token.to_string());
    map.insert("bot_user_id".to_string(), final_id);
    map.insert("bot_username".to_string(), final_name);
    map.insert("bot_app_id".to_string(), final_app_id);
    Ok(map)
}

async fn read_discord_bot_ids_manually(bot_token: &str) -> Result<HashMap<String, String>, String> {
    println!("Enter your Discord Bot User ID:");
    print!("> ");
    let _ = stdout().flush();
    let mut line_id = String::new();
    stdin().read_line(&mut line_id).map_err(|e| e.to_string())?;
    let final_id = line_id.trim().to_string();
    if final_id.is_empty() {
        return Err("Discord: no bot_user_id provided.".into());
    }

    println!("Enter your Discord Bot Username:");
    print!("> ");
    let _ = stdout().flush();
    let mut line_un = String::new();
    stdin().read_line(&mut line_un).map_err(|e| e.to_string())?;
    let final_name = line_un.trim().to_string();
    if final_name.is_empty() {
        return Err("Discord: no bot_username provided.".into());
    }

    println!("Enter your Discord Application ID (App ID):");
    print!("> ");
    let _ = stdout().flush();
    let mut line_app = String::new();
    stdin().read_line(&mut line_app).map_err(|e| e.to_string())?;
    let final_app_id = line_app.trim().to_string();

    let mut map = HashMap::new();
    map.insert("bot_token".to_string(), bot_token.to_string());
    map.insert("bot_user_id".to_string(), final_id);
    map.insert("bot_username".to_string(), final_name);
    map.insert("bot_app_id".to_string(), final_app_id);
    Ok(map)
}

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
            Ok(_) => Ok::<(), Box<dyn std::error::Error>>(()),
            Err(_) => {
                std::process::Command::new("chromium")
                    .arg("--incognito")
                    .arg(url)
                    .spawn()?;
                Ok(())
            }
        };
        return Ok(());
    }

    // fallback:
    Err("Incognito opening not implemented for this platform.".into())
}

async fn update_credential_flags(
    client: &GrpcClient,
    credential_id: &str,
    is_bot: bool,
    is_broadcaster: bool,
    is_teammate: bool,
) -> Result<(), String> {
    use maowbot_proto::maowbot::services::{GetCredentialRequest, StoreCredentialRequest};
    
    // Get the credential first
    let mut cred_client = client.credential.clone();
    let get_request = GetCredentialRequest {
        credential_id: credential_id.to_string(),
    };
    
    let get_response = cred_client
        .get_credential(get_request)
        .await
        .map_err(|e| e.to_string())?;
        
    let credential_info = get_response.into_inner().credential
        .ok_or("No credential found")?;
    let mut credential = credential_info.credential
        .ok_or("No credential in info")?;
        
    // Update flags
    credential.is_bot = is_bot;
    credential.is_broadcaster = is_broadcaster;
    credential.is_teammate = is_teammate;
    
    // Store updated credential
    let store_request = StoreCredentialRequest {
        credential: Some(credential),
        update_if_exists: true,
    };
    
    cred_client
        .store_credential(store_request)
        .await
        .map_err(|e| e.to_string())?;
        
    Ok(())
}

async fn reuse_twitch_helix_for_eventsub(
    client: &GrpcClient,
    user_id: String,
) -> Result<(), String> {
    use maowbot_proto::maowbot::services::StoreCredentialRequest;
    use chrono::Utc;
    
    // Get existing Twitch Helix credential
    let mut cred_client = client.credential.clone();
    let list_request = ListCredentialsRequest {
        platforms: vec![Platform::TwitchHelix as i32],
        active_only: false,
        include_expired: true,
        page: None,
    };
    
    let list_response = cred_client
        .list_credentials(list_request)
        .await
        .map_err(|e| e.to_string())?;
        
    let helix_cred = list_response.into_inner().credentials
        .into_iter()
        .find(|info| {
            info.credential.as_ref()
                .map(|c| c.user_id == user_id)
                .unwrap_or(false)
        })
        .and_then(|info| info.credential)
        .ok_or("No Twitch Helix credential found for user")?;
        
    // Create EventSub credential based on Helix
    let mut eventsub_cred = helix_cred.clone();
    eventsub_cred.platform = Platform::TwitchEventsub as i32;
    eventsub_cred.credential_id = uuid::Uuid::new_v4().to_string();
    eventsub_cred.created_at = Some(prost_types::Timestamp {
        seconds: Utc::now().timestamp(),
        nanos: 0,
    });
    eventsub_cred.last_refreshed = eventsub_cred.created_at.clone();
    
    // Store the new credential
    let store_request = StoreCredentialRequest {
        credential: Some(eventsub_cred),
        update_if_exists: false,
    };
    
    cred_client
        .store_credential(store_request)
        .await
        .map_err(|e| e.to_string())?;
        
    Ok(())
}

fn parse_platform(platform_str: &str) -> Result<Platform, String> {
    match platform_str.to_lowercase().as_str() {
        "twitch" | "twitch-helix" => Ok(Platform::TwitchHelix),
        "twitch-irc" => Ok(Platform::TwitchIrc),
        "twitch-eventsub" => Ok(Platform::TwitchEventsub),
        "discord" => Ok(Platform::Discord),
        "vrchat" => Ok(Platform::Vrchat),
        "vrchat-pipeline" => Ok(Platform::VrchatPipeline),
        _ => Err(format!("Unknown platform '{}'", platform_str)),
    }
}