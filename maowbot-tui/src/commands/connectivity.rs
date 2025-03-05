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
"#.to_string();
    }

    match args[0] {
        "autostart" => handle_autostart_cmd(&args[1..], bot_api).await,
        "start"     => handle_start_cmd(&args[1..], bot_api, tui_module).await,
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

async fn handle_start_cmd(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return "Usage: start <platform> [account]".to_string();
    }

    let platform_str = args[0];
    let platform_enum = match Platform::from_str(platform_str) {
        Ok(p) => p,
        Err(_) => return format!("Unknown platform '{}'", platform_str),
    };

    // If user specified an account explicitly
    if args.len() >= 2 {
        let account = args[1];
        if let Err(e) = bot_api.start_platform_runtime(platform_str, account).await {
            return format!("Error => {:?}", e);
        }

        // If Twitch IRC => auto-join broadcaster & secondary channels
        if platform_str.eq_ignore_ascii_case("twitch-irc") {
            join_broadcaster_and_secondary(bot_api, tui_module, account).await;
        }

        return format!("Started platform='{}', account='{}'", platform_str, account);
    }

    // No account specified => either 0, 1, or multiple credentials might exist
    let all_creds = match bot_api.list_credentials(Some(platform_enum)).await {
        Ok(list) => list,
        Err(e) => return format!("Error listing credentials => {:?}", e),
    };
    if all_creds.is_empty() {
        return format!("No accounts found for platform='{}'. Cannot start.", platform_str);
    } else if all_creds.len() == 1 {
        let c = &all_creds[0];
        let user_display = match bot_api.get_user(c.user_id).await {
            Ok(Some(u)) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
            _ => c.user_id.to_string(),
        };

        if let Err(e) = bot_api.start_platform_runtime(platform_str, &user_display).await {
            return format!("Error => {:?}", e);
        }

        if platform_str.eq_ignore_ascii_case("twitch-irc") {
            join_broadcaster_and_secondary(bot_api, tui_module, &user_display).await;
        }

        return format!(
            "Started platform='{}' with the only account='{}'",
            platform_str, user_display
        );
    }

    // Multiple accounts => prompt user
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
    print!("Press ENTER to start all, or choose an account number to start (default=1): ");
    let _ = stdout().flush();

    let mut line = String::new();
    if stdin().read_line(&mut line).is_err() {
        return "Error reading choice from stdin.".to_string();
    }
    let trimmed = line.trim().to_string();

    // If user just pressed ENTER => start all
    if trimmed.is_empty() {
        for name in &display_list {
            if let Err(e) = bot_api.start_platform_runtime(platform_str, name).await {
                eprintln!("Error starting '{}': {:?}", name, e);
                continue;
            }
            if platform_str.eq_ignore_ascii_case("twitch-irc") {
                join_broadcaster_and_secondary(bot_api, tui_module, name).await;
            }
            println!("Started account='{}'", name);
        }
        return format!("Started ALL accounts ({}) for platform='{}'.", display_list.len(), platform_str);
    }

    // Otherwise, parse user input
    let choice = match trimmed.parse::<usize>() {
        Ok(n) if n > 0 && n <= display_list.len() => n,
        _ => {
            // fallback: assume '1'
            1
        }
    };

    let chosen_account = &display_list[choice - 1];
    if let Err(e) = bot_api.start_platform_runtime(platform_str, chosen_account).await {
        return format!("Error => {:?}", e);
    }
    if platform_str.eq_ignore_ascii_case("twitch-irc") {
        join_broadcaster_and_secondary(bot_api, tui_module, chosen_account).await;
    }
    format!("Started platform='{}', account='{}'", platform_str, chosen_account)
}

/// If the platform is "twitch-irc", we automatically join the configured broadcaster and secondary channels.
async fn join_broadcaster_and_secondary(
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
    account_name: &str
) {
    let (broadcaster_chan, secondary_chan) = {
        let st = tui_module.ttv_state.lock().unwrap();
        (st.broadcaster_channel.clone(), st.secondary_channel.clone())
    };

    for ch_opt in &[broadcaster_chan, secondary_chan] {
        if let Some(ch) = ch_opt {
            let chan_name = if ch.starts_with('#') {
                ch.clone()
            } else {
                format!("#{}", ch)
            };
            {
                // add to joined_channels if not present
                let mut st = tui_module.ttv_state.lock().unwrap();
                if !st.joined_channels.iter().any(|c| c.eq_ignore_ascii_case(&chan_name)) {
                    st.joined_channels.push(chan_name.clone());
                }
            }
            if let Err(e) = bot_api.join_twitch_irc_channel(account_name, &chan_name).await {
                eprintln!("(Warning) Could not auto-join '{}': {:?}", chan_name, e);
            } else {
                println!("Auto-joined '{}'", chan_name);
            }
        }
    }

    // Also set the TUI's "active_account" to whichever we started last
    {
        let mut st = tui_module.ttv_state.lock().unwrap();
        st.active_account = Some(account_name.to_string());
    }
}

async fn handle_stop_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: stop <platform> [account]".to_string();
    }

    let platform_str = args[0];
    let platform_enum = match Platform::from_str(platform_str) {
        Ok(p) => p,
        Err(_) => return format!("Unknown platform '{}'", platform_str),
    };

    // If user provided an account
    if args.len() >= 2 {
        let account = args[1];
        return match bot_api.stop_platform_runtime(platform_str, account).await {
            Ok(_) => format!("Stopped platform='{}', account='{}'", platform_str, account),
            Err(e) => format!("Error => {:?}", e),
        };
    }

    // Otherwise, no account => auto-detect
    let all_creds = match bot_api.list_credentials(Some(platform_enum)).await {
        Ok(list) => list,
        Err(e) => return format!("Error listing credentials => {:?}", e),
    };
    if all_creds.is_empty() {
        return format!("No accounts found for platform='{}'. Cannot stop.", platform_str);
    } else if all_creds.len() == 1 {
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

    // If multiple, prompt
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