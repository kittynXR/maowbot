use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;
use open;
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::error::Error as CoreError;

// We still import callback_server so TUI can spawn the local server.
use maowbot_core::auth::callback_server;
use maowbot_core::auth::callback_server::start_callback_server;

enum PlatformAuthRequirements {
    OnlyClientId {
        dev_console_url: &'static str,
        label_hint: &'static str,
    },
    ClientIdAndSecret {
        dev_console_url: &'static str,
        label_hint: &'static str,
    },
    NoAppCredentials,
}

/// Decide what the TUI should prompt for when creating an auth_config row
/// for each platform.
fn get_auth_requirements(platform: &Platform) -> PlatformAuthRequirements {
    match platform {
        // Updated: Twitch now needs both client_id + client_secret
        // because we have to register as a private app for EventSub.
        Platform::Twitch => {
            PlatformAuthRequirements::ClientIdAndSecret {
                dev_console_url: "https://dev.twitch.tv/console/apps",
                label_hint: "user",
            }
        }
        Platform::Discord => {
            PlatformAuthRequirements::ClientIdAndSecret {
                dev_console_url: "https://discord.com/developers/applications",
                label_hint: "bot",
            }
        }
        Platform::VRChat => {
            PlatformAuthRequirements::NoAppCredentials
        }
        Platform::TwitchIRC => {
            PlatformAuthRequirements::NoAppCredentials
        }
    }
}

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

/// Handles `auth add <platform>`
fn auth_add_flow(platform: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    println!("Is this a bot account? (y/n):");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let is_bot = line.trim().eq_ignore_ascii_case("y");

    let label = prompt_for_label(&platform, is_bot, bot_api);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // ----------------------------------------------------------------
    // Only the TUI code spawns the local server on port=9876
    // ----------------------------------------------------------------
    let fixed_port: u16 = 9876;
    let (done_rx, shutdown_tx) = match rt.block_on(start_callback_server(fixed_port)) {
        Ok(pair) => pair,
        Err(e) => return format!("Error starting callback server => {:?}", e),
    };

    // ----------------------------------------------------------------
    // Attempt to begin the OAuth flow
    // ----------------------------------------------------------------
    let url_result = rt.block_on(
        bot_api.begin_auth_flow_with_label(platform.clone(), is_bot, &label)
    );
    let url = match url_result {
        Ok(u) => u,
        Err(CoreError::Auth(msg)) if msg.contains("No auth_config row found") => {
            // If we have no auth_config, we prompt user to create one interactively
            println!(
                "No auth_config found for (platform={:?}, label='{}'). Let's create a new row.",
                platform, label
            );
            if let Err(e) = create_new_auth_config_interactive(&platform, &label, bot_api) {
                shutdown_tx.send(()).ok();
                return format!("Error creating auth_config => {}", e);
            }
            // Now try again
            match rt.block_on(bot_api.begin_auth_flow_with_label(platform.clone(), is_bot, &label)) {
                Ok(u) => u,
                Err(e) => {
                    shutdown_tx.send(()).ok();
                    return format!("Error after creating auth_config => {:?}", e);
                }
            }
        }
        Err(e) => {
            shutdown_tx.send(()).ok();
            return format!("Error beginning auth flow => {:?}", e);
        }
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
    println!("Waiting for the OAuth callback on port {}...", fixed_port);

    // ----------------------------------------------------------------
    // Wait on the oneshot for the code
    // ----------------------------------------------------------------
    let callback_result = match done_rx.blocking_recv() {
        Ok(res) => res,
        Err(e) => {
            shutdown_tx.send(()).ok();
            return format!("Error receiving OAuth code => {:?}", e);
        }
    };

    // We can shut down the server now
    shutdown_tx.send(()).ok();

    // ----------------------------------------------------------------
    // Complete the auth
    // ----------------------------------------------------------------
    match rt.block_on(bot_api.complete_auth_flow(platform.clone(), callback_result.code)) {
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

/// Handles `auth restart <platform> <user_id>`
fn auth_restart_flow(platform: Platform, user_id: &str, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return format!("Error creating tokio runtime: {:?}", e),
    };

    // Revoke existing
    match rt.block_on(bot_api.revoke_credentials(platform.clone(), user_id)) {
        Ok(_) => {
            println!("Removed old credentials for platform={:?}, user_id={}", platform, user_id);
        }
        Err(e) => {
            println!("Warning: could not revoke old credentials => {:?}", e);
        }
    }

    // Then re-run the standard `auth add`
    auth_add_flow(platform, bot_api)
}

/// Handles `auth remove <platform> <user_id>`
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

/// Interactively create a new `auth_config` row for this (platform, label).
fn create_new_auth_config_interactive(
    platform: &Platform,
    label: &str,
    bot_api: &Arc<dyn BotApi>,
) -> Result<(), String> {
    let requirements = get_auth_requirements(platform);

    match requirements {
        PlatformAuthRequirements::NoAppCredentials => {
            println!("This platform does not require a client_id or client_secret. Creating an empty row...");
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Error creating tokio runtime: {:?}", e))?;

            if let Err(e) = rt.block_on(
                bot_api.create_auth_config(platform.clone(), label, "".to_string(), None)
            ) {
                return Err(format!("Error creating new auth_config => {:?}", e));
            }
            Ok(())
        }
        PlatformAuthRequirements::OnlyClientId { dev_console_url, .. } => {
            // (If any platform still wants only client_id, we'd do it here.)
            println!("Go to your dev console => {}", dev_console_url);
            println!("Open in browser? (y/n): ");
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
            if line.trim().eq_ignore_ascii_case("y") {
                let _ = open::that(dev_console_url);
            }
            let client_id = prompt("Enter client_id:");
            if client_id.trim().is_empty() {
                return Err("No client_id entered => cannot proceed.".to_string());
            }

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Error creating tokio runtime: {:?}", e))?;
            if let Err(e) = rt.block_on(
                bot_api.create_auth_config(platform.clone(), label, client_id, None)
            ) {
                return Err(format!("Error creating new auth_config => {:?}", e));
            }
            Ok(())
        }
        PlatformAuthRequirements::ClientIdAndSecret { dev_console_url, .. } => {
            println!("Go to your dev console => {}", dev_console_url);
            println!("Open in browser? (y/n): ");
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
            if line.trim().eq_ignore_ascii_case("y") {
                let _ = open::that(dev_console_url);
            }
            let client_id = prompt("Enter client_id:");
            if client_id.trim().is_empty() {
                return Err("No client_id entered => cannot proceed.".to_string());
            }
            let client_secret = prompt("Enter client_secret (if any):");

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Error => {:?}", e))?;
            let secret_for_storage = if client_secret.trim().is_empty() {
                None
            } else {
                Some(client_secret)
            };

            if let Err(e) = rt.block_on(
                bot_api.create_auth_config(platform.clone(), label, client_id, secret_for_storage)
            ) {
                return Err(format!("Error creating new auth_config => {:?}", e));
            }
            Ok(())
        }
    }
}

/// Prompt for a suggested `label`, typically something like "bot1" or "user1".
fn prompt_for_label(platform: &Platform, is_bot: bool, bot_api: &Arc<dyn BotApi>) -> String {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let platform_str = format!("{}", platform);
    let count_res = rt.block_on(bot_api.count_auth_configs_for_platform(platform_str));
    let current_count = count_res.unwrap_or_else(|_| 0);

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

/// Simple console prompt helper
fn prompt(msg: &str) -> String {
    use std::io::Write;
    println!("{}", msg);
    print!("> ");
    let _ = std::io::stdout().flush();

    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim().to_string()
}