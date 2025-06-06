// maowbot-tui/src/commands/connectivity.rs

use std::sync::Arc;
use std::io::{stdin, stdout, Write};
use std::str::FromStr;

use maowbot_common::models::platform::Platform;
// use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::traits::api::BotApi;
use maowbot_core::Error;

use crate::tui_module::TuiModule;

/// Handles "autostart", "start", "stop", "chat" commands from the TUI.
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

    match args[0].to_lowercase().as_str() {
        "autostart" => handle_autostart_cmd(&args[1..], bot_api).await,
        "start"     => handle_start_cmd(&args[1..], bot_api).await,
        "stop"      => handle_stop_cmd(&args[1..], bot_api).await,
        "chat"      => handle_chat_cmd(&args[1..], tui_module).await,
        _ => {
            r#"Unknown connectivity command. See usage:
  autostart
  start
  stop
  chat
"#
                .to_string()
        }
    }
}

async fn handle_autostart_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 3 {
        return "Usage: autostart <on/off> <platform> <account>".to_string();
    }
    let on_off   = args[0];
    let platform = args[1];
    let account  = args[2];

    let on = match on_off.to_lowercase().as_str() {
        "on"  => true,
        "off" => false,
        _ => return "Usage: autostart <on/off> <platform> <account>".to_string(),
    };

    // Use the new AutostartApi trait methods
    if let Err(e) = bot_api.set_autostart(platform, account, on).await {
        return format!("Error setting autostart => {:?}", e);
    }

    if on {
        format!("Autostart enabled for platform='{}', account='{}'", platform, account)
    } else {
        format!("Autostart disabled for platform='{}', account='{}'", platform, account)
    }
}

/// Revised "start" command logic.
/// If user picks an account, we start it. Then if it's "twitch-irc", we automatically
/// join the channels for all *other* twitch-irc credentials. We do not rely on
/// ttv broadcaster/secondary settings anymore.
async fn handle_start_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: start <platform> [account]".to_string();
    }
    let platform_str = args[0];
    let platform_enum = match Platform::from_str(platform_str) {
        Ok(p) => p,
        Err(_) => return format!("Unknown platform '{}'", platform_str),
    };

    // If user already specified an account
    if args.len() >= 2 {
        let account = args[1];
        match bot_api.start_platform_runtime(platform_str, account).await {
            Ok(_) => {
                // If it's twitch-irc, do the new "auto-join all other accounts" logic
                if platform_enum == Platform::TwitchIRC {
                    let _ = auto_join_all_other_twitch_accounts(bot_api, account).await;
                }
                format!("Started platform='{}', account='{}'", platform_str, account)
            }
            Err(e) => format!("Error => {:?}", e),
        }
    } else {
        // No account specified => find all credentials
        let all_creds = match bot_api.list_credentials(Some(platform_enum.clone())).await {
            Ok(list) => list,
            Err(e) => return format!("Error listing credentials => {:?}", e),
        };
        if all_creds.is_empty() {
            return format!("No accounts found for platform='{}'. Cannot start.", platform_str);
        } else if all_creds.len() == 1 {
            // Exactly one => proceed
            let c = &all_creds[0];
            let user_display = match bot_api.get_user(c.user_id).await {
                Ok(Some(u)) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
                _ => c.user_id.to_string(),
            };
            match bot_api.start_platform_runtime(platform_str, &user_display).await {
                Ok(_) => {
                    if platform_enum == Platform::TwitchIRC {
                        let _ = auto_join_all_other_twitch_accounts(bot_api, &user_display).await;
                    }
                    format!(
                        "Started platform='{}' with the only account='{}'",
                        platform_str, user_display
                    )
                }
                Err(e) => format!("Error => {:?}", e),
            }
        } else {
            // Multiple => ask user to pick or press ENTER to start all
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

            if trimmed.is_empty() {
                // Start all
                for name in &display_list {
                    if let Err(e) = bot_api.start_platform_runtime(platform_str, name).await {
                        eprintln!("Error starting '{}': {:?}", name, e);
                        continue;
                    }
                    if platform_enum == Platform::TwitchIRC {
                        let _ = auto_join_all_other_twitch_accounts(bot_api, name).await;
                    }
                    println!("Started account='{}'", name);
                }
                format!("Started ALL accounts ({}) for platform='{}'.", display_list.len(), platform_str)
            } else {
                let choice = match trimmed.parse::<usize>() {
                    Ok(n) if n > 0 && n <= display_list.len() => n,
                    _ => 1, // fallback
                };
                let chosen_account = &display_list[choice - 1];
                match bot_api.start_platform_runtime(platform_str, chosen_account).await {
                    Ok(_) => {
                        if platform_enum == Platform::TwitchIRC {
                            let _ = auto_join_all_other_twitch_accounts(bot_api, chosen_account).await;
                        }
                        format!("Started platform='{}', account='{}'", platform_str, chosen_account)
                    }
                    Err(e) => format!("Error => {:?}", e),
                }
            }
        }
    }
}

/// When we start a particular twitch-irc account “X,” automatically have “X” JOIN
/// the channels named after *all the other* twitch-irc credentials. E.g., if we have:
/// - broadcaster => user_name="cuteStreamer"
/// - bot => user_name="myBot"
/// - teammate => user_name="otherModerator"
/// Then whichever we start, it does: JOIN #cuteStreamer, JOIN #myBot, JOIN #otherModerator
/// except for itself.
async fn auto_join_all_other_twitch_accounts(
    bot_api: &Arc<dyn BotApi>,
    started_account: &str
) -> Result<(), Error> {
    // 1) list all twitch-irc credentials
    let all = bot_api.list_credentials(Some(Platform::TwitchIRC)).await?;
    // 2) find the user_name of “started_account” => for clarity, store it in a local var
    let started_cred = all
        .iter()
        .find(|c| c.user_name.eq_ignore_ascii_case(started_account));

    let me = match started_cred {
        Some(sc) => sc,
        None => return Ok(()), // no match => just skip
    };

    // 3) For each other credential, join that user_name as #something
    for c in &all {
        if c.user_name.eq_ignore_ascii_case(&me.user_name) {
            continue; // skip self
        }
        let chan = format!("#{}", c.user_name);
        if let Err(e) = bot_api.join_twitch_irc_channel(&me.user_name, &chan).await {
            eprintln!("(Warning) Could not join '{}' => {:?}", chan, e);
        }
    }

    Ok(())
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

    // No account => auto-detect
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

    // If multiple => prompt
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
