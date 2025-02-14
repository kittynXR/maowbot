// File: maowbot-tui/src/commands/connectivity.rs

use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use std::str::FromStr;

use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::tasks::autostart::AutostartConfig;

use crate::tui_module::TuiModule;

/// Handles "autostart", "start", "stop", "chat" commands asynchronously.
pub async fn handle_connectivity_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return r#"Usage:
              autostart <on/off> <platform> <account>
              start <platform> [account]
              stop <platform> [account]
              chat <on/off> [platform] [account]
            "#
            .to_string();
    }

    match args[0] {
        "autostart" => handle_autostart_cmd(&args[1..], bot_api).await,
        "start"     => handle_start_cmd(&args[1..], bot_api).await,
        "stop"      => handle_stop_cmd(&args[1..], bot_api).await,
        "chat"      => handle_chat_cmd(&args[1..], tui_module).await,
        _ => r#"Unknown connectivity command. See usage:
  autostart
  start
  stop
  chat
"#
            .to_string(),
    }
}

async fn handle_autostart_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 3 {
        return "Usage: autostart <on/off> <platform> <account>".to_string();
    }
    let on_off = args[0];
    let platform = args[1];
    let account = args[2];

    let on = match on_off.to_lowercase().as_str() {
        "on" => true,
        "off" => false,
        _ => return "Usage: autostart <on/off> <platform> <account>".to_string(),
    };

    let current_val = bot_api.get_bot_config_value("autostart").await;
    let config_json = match current_val {
        Ok(Some(s)) => s,
        _ => String::new(),
    };

    let mut config_obj: AutostartConfig = if config_json.is_empty() {
        AutostartConfig::new()
    } else {
        match serde_json::from_str(&config_json) {
            Ok(cfg) => cfg,
            Err(_) => AutostartConfig::new(),
        }
    };

    config_obj.set_platform_account(platform, account, on);

    let new_str = match serde_json::to_string_pretty(&config_obj) {
        Ok(s) => s,
        Err(e) => return format!("Error serializing autostart => {:?}", e),
    };

    if let Err(e) = bot_api.set_bot_config_value("autostart", &new_str).await {
        return format!("Error saving autostart => {:?}", e);
    }

    if on {
        format!("Autostart enabled for platform='{}', account='{}'", platform, account)
    } else {
        format!("Autostart disabled for platform='{}', account='{}'", platform, account)
    }
}

/// If the user provided no account, try to auto‐resolve which account to use for the given platform.
async fn handle_start_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    // Usage: start <platform> [account]
    if args.is_empty() {
        return "Usage: start <platform> [account]".to_string();
    }

    let platform_str = args[0];
    let platform_enum = match Platform::from_str(platform_str) {
        Ok(p) => p,
        Err(_) => return format!("Unknown platform '{}'", platform_str),
    };

    // If user provided an account, just do the old direct logic.
    if args.len() >= 2 {
        let account = args[1];
        return match bot_api.start_platform_runtime(platform_str, account).await {
            Ok(_) => format!("Started platform='{}', account='{}'", platform_str, account),
            Err(e) => format!("Error => {:?}", e),
        };
    }

    // Otherwise, user did NOT provide an account => auto-detect
    let all_creds = match bot_api.list_credentials(Some(platform_enum)).await {
        Ok(list) => list,
        Err(e) => return format!("Error listing credentials => {:?}", e),
    };
    if all_creds.is_empty() {
        return format!("No accounts found for platform='{}'. Cannot start.", platform_str);
    } else if all_creds.len() == 1 {
        // Exactly one → use it automatically
        let c = &all_creds[0];
        let user_display = match bot_api.get_user(c.user_id).await {
            Ok(Some(u)) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
            _ => c.user_id.to_string(),
        };
        return match bot_api.start_platform_runtime(platform_str, &user_display).await {
            Ok(_) => format!(
                "Started platform='{}' with the only account='{}'",
                platform_str, user_display
            ),
            Err(e) => format!("Error => {:?}", e),
        };
    }

    // If multiple, prompt user to pick from a list:
    println!("Multiple accounts found for platform '{}':", platform_str);
    let mut display_list = Vec::new();
    for (idx, cred) in all_creds.iter().enumerate() {
        let user_display = match bot_api.get_user(cred.user_id).await {
            Ok(Some(u)) => u.global_username.unwrap_or_else(|| cred.user_id.to_string()),
            _ => cred.user_id.to_string(),
        };
        println!("  [{}] {}", idx + 1, user_display);
        display_list.push(user_display);
    }
    print!("Select an account number to start: ");
    let _ = stdout().flush();

    let mut line = String::new();
    if stdin().read_line(&mut line).is_err() {
        return "Error reading choice from stdin.".to_string();
    }
    let trimmed = line.trim();
    let choice = match trimmed.parse::<usize>() {
        Ok(n) if n > 0 && n <= display_list.len() => n,
        _ => return "Invalid choice. Aborting.".to_string(),
    };
    let chosen_account = &display_list[choice - 1];
    match bot_api.start_platform_runtime(platform_str, chosen_account).await {
        Ok(_) => format!("Started platform='{}', account='{}'", platform_str, chosen_account),
        Err(e) => format!("Error => {:?}", e),
    }
}

/// If the user provided no account, try to auto‐resolve which account to use for the given platform.
async fn handle_stop_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    // Usage: stop <platform> [account]
    if args.is_empty() {
        return "Usage: stop <platform> [account]".to_string();
    }

    let platform_str = args[0];
    let platform_enum = match Platform::from_str(platform_str) {
        Ok(p) => p,
        Err(_) => return format!("Unknown platform '{}'", platform_str),
    };

    // If user provided an account, just do the old direct logic.
    if args.len() >= 2 {
        let account = args[1];
        return match bot_api.stop_platform_runtime(platform_str, account).await {
            Ok(_) => format!("Stopped platform='{}', account='{}'", platform_str, account),
            Err(e) => format!("Error => {:?}", e),
        };
    }

    // Otherwise, user did NOT provide an account => auto-detect
    let all_creds = match bot_api.list_credentials(Some(platform_enum)).await {
        Ok(list) => list,
        Err(e) => return format!("Error listing credentials => {:?}", e),
    };
    if all_creds.is_empty() {
        return format!("No accounts found for platform='{}'. Cannot stop.", platform_str);
    } else if all_creds.len() == 1 {
        // Exactly one → use it automatically
        let c = &all_creds[0];
        let user_display = match bot_api.get_user(c.user_id).await {
            Ok(Some(u)) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
            _ => c.user_id.to_string(),
        };
        return match bot_api.stop_platform_runtime(platform_str, &user_display).await {
            Ok(_) => format!(
                "Stopped platform='{}' with the only account='{}'",
                platform_str, user_display
            ),
            Err(e) => format!("Error => {:?}", e),
        };
    }

    // If multiple, prompt user to pick from a list:
    println!("Multiple accounts found for platform '{}':", platform_str);
    let mut display_list = Vec::new();
    for (idx, cred) in all_creds.iter().enumerate() {
        let user_display = match bot_api.get_user(cred.user_id).await {
            Ok(Some(u)) => u.global_username.unwrap_or_else(|| cred.user_id.to_string()),
            _ => cred.user_id.to_string(),
        };
        println!("  [{}] {}", idx + 1, user_display);
        display_list.push(user_display);
    }
    print!("Select an account number to stop: ");
    let _ = stdout().flush();

    let mut line = String::new();
    if stdin().read_line(&mut line).is_err() {
        return "Error reading choice from stdin.".to_string();
    }
    let trimmed = line.trim();
    let choice = match trimmed.parse::<usize>() {
        Ok(n) if n > 0 && n <= display_list.len() => n,
        _ => return "Invalid choice. Aborting.".to_string(),
    };
    let chosen_account = &display_list[choice - 1];
    match bot_api.stop_platform_runtime(platform_str, chosen_account).await {
        Ok(_) => format!("Stopped platform='{}', account='{}'", platform_str, chosen_account),
        Err(e) => format!("Error => {:?}", e),
    }
}

/// chat <on/off> [platform] [account]
async fn handle_chat_cmd(args: &[&str], tui_module: &Arc<TuiModule>) -> String {
    if args.is_empty() {
        return "Usage: chat <on/off> [platform] [account]".to_string();
    }
    let on_off = args[0].to_lowercase();
    let on = on_off == "on";

    let (pf, af) = match args.len() {
        1 => (None, None),
        2 => (Some(args[1].to_string()), None),
        _ => (Some(args[1].to_string()), Some(args[2].to_string())),
    };

    tui_module.set_chat_state(on, pf.clone(), af.clone()).await;

    if on {
        match (pf, af) {
            (None, None) => "Chat ON for ALL platforms/accounts".to_string(),
            (Some(p), None) => format!("Chat ON for platform='{}' (any account)", p),
            (Some(p), Some(a)) => format!("Chat ON for platform='{}', account='{}'", p, a),
            _ => unreachable!(),
        }
    } else {
        "Chat OFF".to_string()
    }
}