use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;
use open;
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::error::Error as CoreError;

/// Helper to describe what credentials a given platform typically needs.
enum PlatformAuthRequirements {
    /// e.g., Twitch (public or implicit flow) only needs a client_id
    OnlyClientId {
        dev_console_url: &'static str,
        label_hint: &'static str,
    },
    /// e.g., Discord often needs a client_id + client_secret
    ClientIdAndSecret {
        dev_console_url: &'static str,
        label_hint: &'static str,
    },
    /// If some other platform doesn’t require anything at all
    NoAppCredentials,
}

fn get_auth_requirements(platform: &Platform) -> PlatformAuthRequirements {
    match platform {
        // Example: Twitch Helix (public flow)
        Platform::Twitch => {
            PlatformAuthRequirements::OnlyClientId {
                dev_console_url: "https://dev.twitch.tv/console/apps",
                label_hint: "user",
            }
        }
        // Example: Discord typically has both client_id + client_secret
        Platform::Discord => {
            PlatformAuthRequirements::ClientIdAndSecret {
                dev_console_url: "https://discord.com/developers/applications",
                label_hint: "bot",
            }
        }
        // Example: VRChat might need username/password or other approach
        // but for the “auth_config” table we might still store client_id/secret.
        // Tweak as you like:
        Platform::VRChat => {
            PlatformAuthRequirements::NoAppCredentials
        }
        // Example: TwitchIRC might not need an ID/secret in `auth_config` (since the user obtains chat tokens).
        Platform::TwitchIRC => {
            PlatformAuthRequirements::NoAppCredentials
        }
    }
}

/// Main dispatch for the 'auth' subcommands
pub fn handle_auth_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: auth <add|remove|list|restart> [options]".to_string();
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
                    if creds.is_empty() {
                        "No credentials stored.\n".to_string()
                    } else {
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
                }
                Err(e) => format!("Error listing credentials => {:?}", e),
            }
        }

        "restart" => {
            if args.len() < 3 {
                return "Usage: auth restart <platform> <user_id>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(platform) => auth_restart_flow(platform, args[2], bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }

        _ => {
            "Usage: auth <add|remove|list|restart> [platform] [user_id]".to_string()
        }
    }
}

/// Interactive ‘auth add’ flow for a new credential.
fn auth_add_flow(platform: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    println!("Is this a bot account? (y/n):");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    // Prompt user to pick or confirm a label
    let label = prompt_for_label(&platform, is_bot, bot_api);

    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return format!("Error creating tokio runtime: {:?}", e),
    };

    // Step 1) Attempt to begin auth flow
    let url_result = rt.block_on(bot_api.begin_auth_flow_with_label(platform.clone(), is_bot, &label));

    let url = match url_result {
        Ok(u) => {
            // We already have a valid auth flow URL or instructions.
            u
        }
        Err(CoreError::Auth(msg)) if msg.contains("No auth_config row found") => {
            println!(
                "No config found for (platform={:?}, label='{}').",
                platform, label
            );
            // Instead of using `?`, explicitly handle the result:
            if let Err(e) = create_new_auth_config_interactive(&platform, &label, bot_api) {
                return format!("Error creating new auth_config => {}", e);
            }
            // Now that a row exists, try again:
            match rt.block_on(bot_api.begin_auth_flow_with_label(platform.clone(), is_bot, &label)) {
                Ok(u) => u,
                Err(e) => return format!("Error beginning auth flow after creation => {:?}", e),
            }
        }
        Err(e) => {
            return format!("Error beginning auth flow => {:?}", e);
        }
    };

    // If the authenticator gave us a URL, let's prompt user to open it:
    println!("Open this URL to authenticate:\n  {}", url);
    println!("Open in browser now? (y/n):");
    let mut line2 = String::new();
    let _ = std::io::stdin().read_line(&mut line2);
    if line2.trim().eq_ignore_ascii_case("y") {
        if let Err(err) = open::that(&url) {
            println!("Could not open browser automatically: {:?}", err);
        }
    }

    // Ask the user for the code param if relevant
    println!("If you were given a 'code=' param in the callback (or it auto-redirected), enter it here.\n(Press enter if you want to cancel): ");
    let mut code_line = String::new();
    let _ = std::io::stdin().read_line(&mut code_line);
    let code_str = code_line.trim().to_string();

    if code_str.is_empty() {
        // The user pressed enter => treat as a cancellation
        return "Auth flow canceled. No credentials stored.".to_string();
    }

    // Step 2: complete the flow
    match rt.block_on(bot_api.complete_auth_flow(platform.clone(), code_str)) {
        Ok(cred) => {
            format!(
                "Success! Stored credentials for platform={:?}, user_id='{}', is_bot={}, label='{}'",
                cred.platform, cred.user_id, cred.is_bot, label
            )
        }
        Err(e) => {
            format!("Error completing auth => {:?}", e)
        }
    }
}

/// Revoke credentials for <platform, user_id>, then re-run the same interactive flow.
fn auth_restart_flow(platform: Platform, user_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    // 1) Revoke existing credentials (if any)
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return format!("Error creating tokio runtime: {:?}", e),
    };

    match rt.block_on(bot_api.revoke_credentials(platform.clone(), user_id)) {
        Ok(_) => {
            println!(
                "Removed (revoked) old credentials for platform={:?}, user_id={}",
                platform, user_id
            );
        }
        Err(e) => {
            println!(
                "Warning: could not revoke old credentials for platform={:?}, user_id={}: {:?}",
                platform, user_id, e
            );
        }
    }

    // 2) Now run auth_add_flow again
    auth_add_flow(platform, bot_api)
}

/// Revoke (remove) an existing credential
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

/// If no auth_config row is found, we prompt user for the needed fields (client_id, optionally secret),
/// then create the row by calling `bot_api.create_auth_config(...)`.
///
/// Returns Ok(()) or an Err(...) if user cancels.
fn create_new_auth_config_interactive(
    platform: &Platform,
    label: &str,
    bot_api: &Arc<dyn BotApi>,
) -> Result<(), String> {
    let requirements = get_auth_requirements(platform);

    match requirements {
        PlatformAuthRequirements::NoAppCredentials => {
            println!("This platform does not require a client_id or secret. We'll create a blank config row...");
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Error creating tokio runtime: {:?}", e))?;

            // Insert a row with empty strings for client_id, client_secret
            if let Err(e) = rt.block_on(
                bot_api.create_auth_config(platform.clone(), label, "".to_string(), None)
            ) {
                return Err(format!("Error creating new auth_config => {:?}", e));
            }
            Ok(())
        }

        PlatformAuthRequirements::OnlyClientId {
            dev_console_url,
            ..
        } => {
            println!(
                "We can open the dev console now to create/find your client ID:\n  {}",
                dev_console_url
            );
            print!("Open in browser? (y/n): ");
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
            if line.trim().eq_ignore_ascii_case("y") {
                if let Err(err) = open::that(dev_console_url) {
                    println!("(warn) Could not open automatically: {:?}", err);
                }
            }

            // Now ask for the client_id
            let client_id = prompt("Enter client_id:");
            if client_id.trim().is_empty() {
                return Err("No client_id entered => cannot proceed.".to_string());
            }

            // Create row with no secret
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Error creating tokio runtime: {:?}", e))?;
            if let Err(e) = rt.block_on(
                bot_api.create_auth_config(
                    platform.clone(),
                    label,
                    client_id,
                    None,
                )
            ) {
                return Err(format!("Error creating new auth_config => {:?}", e));
            }
            Ok(())
        }

        PlatformAuthRequirements::ClientIdAndSecret {
            dev_console_url,
            ..
        } => {
            println!(
                "We can open the dev console now to create/find your client credentials:\n  {}",
                dev_console_url
            );
            print!("Open in browser? (y/n): ");
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
            if line.trim().eq_ignore_ascii_case("y") {
                if let Err(err) = open::that(dev_console_url) {
                    println!("(warn) Could not open automatically: {:?}", err);
                }
            }

            // Ask for client_id
            let client_id = prompt("Enter client_id:");
            if client_id.trim().is_empty() {
                return Err("No client_id entered => cannot proceed.".to_string());
            }

            // Ask for client_secret
            let client_secret = prompt("Enter client_secret (if any):");
            // Possibly required, possibly not — depends on your usage

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Error creating tokio runtime: {:?}", e))?;
            let secret_for_storage = if client_secret.trim().is_empty() {
                None
            } else {
                Some(client_secret)
            };

            if let Err(e) = rt.block_on(
                bot_api.create_auth_config(
                    platform.clone(),
                    label,
                    client_id,
                    secret_for_storage,
                )
            ) {
                return Err(format!("Error creating new auth_config => {:?}", e));
            }

            Ok(())
        }
    }
}

/// Prompt user for a label to store in `auth_config`, e.g. 'bot1' or 'user2'.
fn prompt_for_label(platform: &Platform, is_bot: bool, bot_api: &Arc<dyn BotApi>) -> String {
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

    // Construct a default label from is_bot + count, e.g. "bot1" or "user1"
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
    use std::io::Write;
    println!("{}", msg);
    print!("> ");
    let _ = std::io::stdout().flush();

    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_string()
}