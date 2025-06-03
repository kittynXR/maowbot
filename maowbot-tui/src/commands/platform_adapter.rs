// Platform command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::platform::PlatformCommands};
use std::io::{stdin, stdout, Write};

pub async fn handle_platform_command(args: &[&str], client: &GrpcClient) -> String {
    if args.is_empty() {
        return "Usage: platform <add|remove|list|show>".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: platform add <platformName>".to_string();
            }
            match parse_platform(args[1]) {
                Ok(plat) => handle_platform_add(plat, client).await,
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "remove" => {
            if args.len() < 2 {
                return "Usage: platform remove <platformName>".to_string();
            }
            match parse_platform(args[1]) {
                Ok(plat) => handle_platform_remove(plat, client).await,
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "list" => {
            let maybe_platform = if args.len() > 1 {
                parse_platform(args[1]).ok()
            } else {
                None
            };
            
            let platforms = if let Some(plat) = maybe_platform {
                vec![plat]
            } else {
                vec![]
            };
            
            match PlatformCommands::list_platform_configs(client, platforms, 100).await {
                Ok(result) => {
                    if result.data.configs.is_empty() {
                        "No platform configs found.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Current platform configs:\n");
                        for pc in &result.data.configs {
                            out.push_str(&format!(
                                " - id={} platform={} client_id={}\n",
                                pc.platform_config_id,
                                format_platform_str(&pc.platform),
                                pc.client_id,
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing platform configs => {:?}", e),
            }
        }
        "show" => {
            if args.len() < 2 {
                return "Usage: platform show <platformName>".to_string();
            }
            match parse_platform(args[1]) {
                Ok(plat) => platform_show(plat, client).await,
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        _ => "Usage: platform <add|remove|list|show>".to_string(),
    }
}

async fn handle_platform_add(plat: i32, client: &GrpcClient) -> String {
    let platform_str = format_platform(plat);

    // If VRChat, we just set the default API key and skip all user input:
    if plat == 4 { // PLATFORM_VRCHAT
        // Insert VRChat default API key
        let vrchat_default_key = "JlE5Jldo5Jibnk5O5hTx6XVqsJu4WJ26".to_string();
        
        match PlatformCommands::create_platform_config(
            client,
            plat,
            &vrchat_default_key,
            None,
            vec![]
        ).await {
            Ok(_) => format!("VRChat platform config created with default API key."),
            Err(e) => format!("Error storing VRChat config => {}", e),
        }
    } else {
        // For other platforms, continue the usual flow:
        println!("You are adding/updating the platform config for '{}'.", platform_str);

        let also_add_irc_and_eventsub = plat == 6; // PLATFORM_TWITCH_HELIX

        let dev_console_url = match plat {
            6 => Some("https://dev.twitch.tv/console"), // PLATFORM_TWITCH_HELIX
            3 => Some("https://discord.com/developers/applications"), // PLATFORM_DISCORD
            4 => Some("https://dashboard.vrchat.com/"), // PLATFORM_VRCHAT
            _ => None,
        };
        
        if let Some(url) = dev_console_url {
            println!("Open the dev console for {} now? (y/n):", platform_str);
            print!("> ");
            let _ = stdout().flush();
            let mut line = String::new();
            let _ = stdin().read_line(&mut line);
            if line.trim().eq_ignore_ascii_case("y") {
                let _ = open::that(url);
            }
        }

        println!("Enter client_id (or leave blank if not needed): ");
        print!("> ");
        let _ = stdout().flush();
        let mut cid = String::new();
        let _ = stdin().read_line(&mut cid);
        let client_id = cid.trim().to_string();

        println!("Enter client_secret (or leave blank if not needed): ");
        print!("> ");
        let _ = stdout().flush();
        let mut csec = String::new();
        let _ = stdin().read_line(&mut csec);
        let client_secret = csec.trim().to_string();
        let secret_opt = if client_secret.is_empty() { None } else { Some(client_secret.as_str()) };

        match PlatformCommands::create_platform_config(client, plat, &client_id, secret_opt, vec![]).await {
            Ok(_) => {
                let mut msg = format!("Platform config created for '{}'.", platform_str);
                
                if also_add_irc_and_eventsub {
                    // Also create twitch-irc and twitch-eventsub
                    let _ = PlatformCommands::create_platform_config(
                        client,
                        1, // PLATFORM_TWITCH_IRC
                        &client_id,
                        secret_opt,
                        vec![]
                    ).await.map_err(|e| {
                        println!("(Warning) could not create 'twitch-irc': {}", e)
                    });

                    let _ = PlatformCommands::create_platform_config(
                        client,
                        2, // PLATFORM_TWITCH_EVENTSUB
                        &client_id,
                        secret_opt,
                        vec![]
                    ).await.map_err(|e| {
                        println!("(Warning) could not create 'twitch-eventsub': {}", e)
                    });
                    
                    msg.push_str("\nAlso created configs for twitch-irc and twitch-eventsub.");
                }
                
                msg
            }
            Err(e) => format!("Error storing config for '{}': {}", platform_str, e),
        }
    }
}

async fn handle_platform_remove(plat: i32, client: &GrpcClient) -> String {
    let platform_str = format_platform(plat);
    println!("Removing platform config(s) for '{}'.", platform_str);

    let also_remove_irc_and_eventsub = plat == 6; // PLATFORM_TWITCH_HELIX

    // List all configs first
    match PlatformCommands::list_platform_configs(client, vec![], 100).await {
        Ok(result) => {
            if result.data.configs.is_empty() {
                return "No platform configs found in the database.".to_string();
            }

            // Find matching configs
            let matching: Vec<_> = result.data.configs.iter()
                .filter(|pc| pc.platform == plat.to_string())
                .collect();
                
            if matching.is_empty() {
                return format!("No platform config found for '{}'.", platform_str);
            }

            // TODO: Check for existing credentials before allowing deletion
            // This would require CredentialService integration

            println!("\nExisting config(s) for '{}':", platform_str);
            for pc in &matching {
                println!(
                    " - id={} platform={} client_id={}",
                    pc.platform_config_id,
                    format_platform_str(&pc.platform),
                    pc.client_id
                );
            }
            
            println!("Enter the platform_config_id to remove (or leave blank to skip): ");
            print!("> ");
            let _ = stdout().flush();

            let mut line = String::new();
            let _ = stdin().read_line(&mut line);
            let chosen_id = line.trim().to_string();
            
            if chosen_id.is_empty() {
                return format!("Skipped removal for '{}'.", platform_str);
            }

            match PlatformCommands::delete_platform_config(client, &chosen_id).await {
                Ok(_) => {
                    let mut msg = format!("Removed platform config with id={}.", chosen_id);
                    
                    if also_remove_irc_and_eventsub {
                        // Try to remove related configs
                        for pc in &result.data.configs {
                            if (pc.platform == "1" || // PLATFORM_TWITCH_IRC
                                pc.platform == "2") && // PLATFORM_TWITCH_EVENTSUB
                               pc.client_id == matching[0].client_id {
                                let _ = PlatformCommands::delete_platform_config(
                                    client,
                                    &pc.platform_config_id
                                ).await;
                            }
                        }
                        msg.push_str("\nAlso attempted to remove twitch-irc and twitch-eventsub configs.");
                    }
                    
                    msg
                }
                Err(e) => format!("Error removing => {}", e),
            }
        }
        Err(e) => format!("Error listing platform configs => {}", e),
    }
}

async fn platform_show(plat: i32, client: &GrpcClient) -> String {
    let platform_str = format_platform(plat);
    
    match PlatformCommands::list_platform_configs(client, vec![plat], 1).await {
        Ok(result) => {
            if result.data.configs.is_empty() {
                format!("No platform config found for '{}'.", platform_str)
            } else {
                let pc = &result.data.configs[0];
                let mut out = String::new();
                out.push_str(&format!("platform={} (id={})\n", format_platform_str(&pc.platform), pc.platform_config_id));
                out.push_str(&format!("client_id='{}'\n", pc.client_id));
                out.push_str(&format!("client_secret='{}'\n", if pc.encrypted_client_secret.is_empty() { "NONE" } else { "***" }));
                out
            }
        }
        Err(e) => format!("Error => {}", e),
    }
}

// Helper functions
fn parse_platform(s: &str) -> Result<i32, String> {
    let plat = match s.to_lowercase().as_str() {
        "twitch" | "twitch-helix" => 6, // PLATFORM_TWITCH_HELIX
        "twitch-irc" | "twitchirc" => 1, // PLATFORM_TWITCH_IRC
        "twitch-eventsub" | "twitcheventsub" => 2, // PLATFORM_TWITCH_EVENTSUB
        "discord" => 3, // PLATFORM_DISCORD
        "vrchat" => 4, // PLATFORM_VRCHAT
        "vrchat-pipeline" | "vrchatpipeline" => 5, // PLATFORM_VRCHAT_PIPELINE
        _ => return Err(format!("Unknown platform: {}", s)),
    };
    Ok(plat)
}

fn format_platform(plat: i32) -> String {
    match plat {
        0 => "unknown",
        6 => "twitch",
        1 => "twitch-irc",
        2 => "twitch-eventsub",
        3 => "discord",
        4 => "vrchat",
        5 => "vrchat-pipeline",
        _ => "unknown",
    }.to_string()
}

fn format_platform_str(plat: &str) -> String {
    match plat {
        "0" => "unknown",
        "6" => "twitch",
        "1" => "twitch-irc",
        "2" => "twitch-eventsub",
        "3" => "discord",
        "4" => "vrchat",
        "5" => "vrchat-pipeline",
        _ => "unknown",
    }.to_string()
}