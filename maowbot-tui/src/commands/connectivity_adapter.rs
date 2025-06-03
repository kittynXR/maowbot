// Connectivity command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::connectivity::ConnectivityCommands};
use std::io::{stdin, stdout, Write};
use crate::tui_module_simple::SimpleTuiModule;
use std::sync::Arc;

pub async fn handle_connectivity_command(
    args: &[&str],
    client: &GrpcClient,
    tui_module: &Arc<SimpleTuiModule>,
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
        "autostart" => handle_autostart_cmd(&args[1..], client).await,
        "start"     => handle_start_cmd(&args[1..], client).await,
        "stop"      => handle_stop_cmd(&args[1..], client).await,
        "chat"      => handle_chat_cmd(&args[1..], tui_module).await,
        _ => {
            r#"Unknown connectivity command. See usage:
  autostart
  start
  stop
  chat
"#.to_string()
        }
    }
}

async fn handle_autostart_cmd(args: &[&str], client: &GrpcClient) -> String {
    if args.len() < 3 {
        return "Usage: autostart <on/off> <platform> <account>".to_string();
    }
    let on_off   = args[0];
    let platform = args[1];
    let account  = args[2];

    let enable = match on_off.to_lowercase().as_str() {
        "on"  => true,
        "off" => false,
        _ => return "Usage: autostart <on/off> <platform> <account>".to_string(),
    };

    match ConnectivityCommands::configure_autostart(client, enable, platform, account).await {
        Ok(result) => {
            if result.enabled {
                format!("Autostart enabled for platform='{}', account='{}'", result.platform, result.account)
            } else {
                format!("Autostart disabled for platform='{}', account='{}'", result.platform, result.account)
            }
        }
        Err(e) => format!("Error configuring autostart => {}", e),
    }
}

async fn handle_start_cmd(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: start <platform> [account]".to_string();
    }
    let platform_str = args[0];

    // If user already specified an account
    if args.len() >= 2 {
        let account = args[1];
        match ConnectivityCommands::start_platform(client, platform_str, account).await {
            Ok(result) => {
                // If it's twitch-irc, do the auto-join logic
                if platform_str.eq_ignore_ascii_case("twitch-irc") {
                    let _ = auto_join_all_other_twitch_accounts(client, account).await;
                }
                format!("Started platform='{}', account='{}'", result.platform, result.account)
            }
            Err(e) => format!("Error => {}", e),
        }
    } else {
        // No account specified => find all credentials
        let accounts = match ConnectivityCommands::list_platform_accounts(client, platform_str).await {
            Ok(list) => list,
            Err(e) => return format!("Error listing accounts => {}", e),
        };
        
        if accounts.is_empty() {
            return format!("No accounts found for platform='{}'. Cannot start.", platform_str);
        } else if accounts.len() == 1 {
            // Exactly one => proceed
            let acc = &accounts[0];
            match ConnectivityCommands::start_platform(client, platform_str, &acc.display_name).await {
                Ok(result) => {
                    if platform_str.eq_ignore_ascii_case("twitch-irc") {
                        let _ = auto_join_all_other_twitch_accounts(client, &acc.display_name).await;
                    }
                    format!(
                        "Started platform='{}' with the only account='{}'",
                        result.platform, acc.display_name
                    )
                }
                Err(e) => format!("Error => {}", e),
            }
        } else {
            // Multiple => ask user to pick or press ENTER to start all
            println!("Multiple accounts found for platform '{}':", platform_str);
            let mut display_list = Vec::new();
            for (idx, acc) in accounts.iter().enumerate() {
                println!("  [{}] {}", idx + 1, acc.display_name);
                display_list.push(acc.display_name.clone());
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
                let mut results = Vec::new();
                for name in &display_list {
                    match ConnectivityCommands::start_platform(client, platform_str, name).await {
                        Ok(_) => {
                            if platform_str.eq_ignore_ascii_case("twitch-irc") {
                                let _ = auto_join_all_other_twitch_accounts(client, name).await;
                            }
                            results.push(format!("Started account='{}'", name));
                        }
                        Err(e) => {
                            results.push(format!("Error starting '{}': {}", name, e));
                        }
                    }
                }
                results.join("\n") + &format!("\nStarted {} accounts for platform='{}'.", display_list.len(), platform_str)
            } else {
                let choice = match trimmed.parse::<usize>() {
                    Ok(n) if n > 0 && n <= display_list.len() => n,
                    _ => 1, // fallback
                };
                let chosen_account = &display_list[choice - 1];
                match ConnectivityCommands::start_platform(client, platform_str, chosen_account).await {
                    Ok(result) => {
                        if platform_str.eq_ignore_ascii_case("twitch-irc") {
                            let _ = auto_join_all_other_twitch_accounts(client, chosen_account).await;
                        }
                        format!("Started platform='{}', account='{}'", result.platform, chosen_account)
                    }
                    Err(e) => format!("Error => {}", e),
                }
            }
        }
    }
}

async fn handle_stop_cmd(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: stop <platform> [account]".to_string();
    }
    let platform_str = args[0];

    // If user provided an account
    if args.len() >= 2 {
        let account = args[1];
        match ConnectivityCommands::stop_platform(client, platform_str, account).await {
            Ok(result) => format!("Stopped platform='{}', account='{}'", result.platform, result.account),
            Err(e) => format!("Error => {}", e),
        }
    } else {
        // No account => auto-detect
        let accounts = match ConnectivityCommands::list_platform_accounts(client, platform_str).await {
            Ok(list) => list,
            Err(e) => return format!("Error listing accounts => {}", e),
        };
        
        if accounts.is_empty() {
            return format!("No accounts found for platform='{}'. Cannot stop.", platform_str);
        } else if accounts.len() == 1 {
            let acc = &accounts[0];
            match ConnectivityCommands::stop_platform(client, platform_str, &acc.display_name).await {
                Ok(result) => format!(
                    "Stopped platform='{}' with the only account='{}'",
                    result.platform, acc.display_name
                ),
                Err(e) => format!("Error => {}", e),
            }
        } else {
            // If multiple => prompt
            println!("Multiple accounts found for platform '{}':", platform_str);
            let mut display_list = Vec::new();
            for (idx, acc) in accounts.iter().enumerate() {
                println!("  [{}] {}", idx + 1, acc.display_name);
                display_list.push(acc.display_name.clone());
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
            match ConnectivityCommands::stop_platform(client, platform_str, chosen_account).await {
                Ok(result) => format!("Stopped platform='{}', account='{}'", result.platform, chosen_account),
                Err(e) => format!("Error => {}", e),
            }
        }
    }
}

async fn handle_chat_cmd(args: &[&str], tui_module: &Arc<SimpleTuiModule>) -> String {
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

/// Auto-join all other Twitch IRC accounts' channels
async fn auto_join_all_other_twitch_accounts(
    client: &GrpcClient,
    started_account: &str
) -> Result<(), String> {
    // List all twitch-irc credentials
    let all = match ConnectivityCommands::list_platform_accounts(client, "twitch-irc").await {
        Ok(list) => list,
        Err(e) => return Err(format!("Failed to list accounts: {}", e)),
    };
    
    // Find the started account
    let me = match all.iter().find(|a| a.user_name.eq_ignore_ascii_case(started_account)) {
        Some(acc) => acc,
        None => return Ok(()), // no match => skip
    };
    
    // For each other credential, join that user_name as #channel
    for acc in &all {
        if acc.user_name.eq_ignore_ascii_case(&me.user_name) {
            continue; // skip self
        }
        let chan = format!("#{}", acc.user_name);
        if let Err(e) = ConnectivityCommands::join_twitch_channel(client, &me.user_name, &chan).await {
            eprintln!("(Warning) Could not join '{}' => {}", chan, e);
        }
    }
    
    Ok(())
}