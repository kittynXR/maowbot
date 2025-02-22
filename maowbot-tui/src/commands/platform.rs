use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::str::FromStr;
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::plugins::bot_api::platform_api::PlatformConfigData;

pub async fn handle_platform_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: platform <add|remove|list|show>".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: platform add <platformName>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(plat) => handle_platform_add(plat, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "remove" => {
            if args.len() < 2 {
                return "Usage: platform remove <platformName>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(plat) => handle_platform_remove(plat, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "list" => {
            let maybe_platform = if args.len() > 1 {
                Some(args[1])
            } else {
                None
            };
            let configs = bot_api.list_platform_configs(maybe_platform).await;
            match configs {
                Ok(list) => {
                    if list.is_empty() {
                        "No platform configs found.\n".to_string()
                    } else {
                        let mut out = String::new();
                        out.push_str("Current platform configs:\n");
                        for pc in list {
                            out.push_str(&format!(
                                " - id={} platform={} client_id={}\n",
                                pc.platform_config_id,
                                pc.platform,
                                pc.client_id.as_deref().unwrap_or("NONE"),
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
            match Platform::from_str(args[1]) {
                Ok(plat) => platform_show(plat, bot_api).await,
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        _ => "Usage: platform <add|remove|list|show>".to_string(),
    }
}

async fn handle_platform_add(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();

    // If VRChat, we just set the default API key and skip all user input:
    if plat == Platform::VRChat {
        // Insert VRChat default API key: "JlE5Jldo5Jibnk5O5hTx6XVqsJu4WJ26"
        let vrchat_default_key = "JlE5Jldo5Jibnk5O5hTx6XVqsJu4WJ26".to_string();
        let res = bot_api
            .create_platform_config(plat.clone(), vrchat_default_key, None)
            .await;
        return match res {
            Ok(_) => format!("VRChat platform config upserted with default API key."),
            Err(e) => format!("Error storing VRChat config => {e}"),
        };
    }

    // For other platforms, continue the usual flow:
    println!("You are adding/updating the platform config for '{}'.", platform_str);

    let also_add_irc_and_eventsub = matches!(plat, Platform::Twitch);

    let dev_console_url = match platform_str.as_str() {
        "twitch" => Some("https://dev.twitch.tv/console"),
        "discord" => Some("https://discord.com/developers/applications"),
        "vrchat" => Some("https://dashboard.vrchat.com/"), // not used if we matched above
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
    let secret_opt = if client_secret.is_empty() { None } else { Some(client_secret) };

    if let Err(e) = bot_api.create_platform_config(plat.clone(), client_id.clone(), secret_opt.clone()).await {
        return format!("Error storing config for '{platform_str}': {e}");
    }

    if also_add_irc_and_eventsub {
        // insert twitch-irc
        let _ = bot_api
            .create_platform_config(Platform::TwitchIRC, client_id.clone(), secret_opt.clone())
            .await
            .map_err(|e| println!("(Warning) could not create 'twitch-irc': {e}"));

        // insert twitch-eventsub
        let _ = bot_api
            .create_platform_config(Platform::TwitchEventSub, client_id.clone(), secret_opt.clone())
            .await
            .map_err(|e| println!("(Warning) could not create 'twitch-eventsub': {e}"));
    }

    format!("Platform config upserted for '{platform_str}'.")
}

async fn handle_platform_remove(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    println!("Removing platform config(s) for '{platform_str}'.");

    let also_remove_irc_and_eventsub = matches!(plat, Platform::Twitch);

    let list = match bot_api.list_platform_configs(None).await {
        Ok(lst) => lst,
        Err(e) => {
            return format!("Error listing platform configs => {e}");
        }
    };
    if list.is_empty() {
        return "No platform configs found in the database.".to_string();
    }

    // remove the main platform's row(s)
    let remove_main = remove_platform_config_by_name(&list, &platform_str, bot_api).await;

    if also_remove_irc_and_eventsub {
        // Attempt to remove twitch-irc
        let _ = remove_platform_config_by_name(&list, "twitch-irc", bot_api).await;
        // Attempt to remove twitch-eventsub
        let _ = remove_platform_config_by_name(&list, "twitch-eventsub", bot_api).await;
    }

    match remove_main {
        Some(msg) => msg,
        None => format!("No platform config found for '{platform_str}'."),
    }
}

async fn remove_platform_config_by_name(
    list: &[PlatformConfigData],
    target_platform_str: &str,
    bot_api: &Arc<dyn BotApi>,
) -> Option<String> {
    let matching: Vec<_> = list
        .iter()
        .filter(|pc| pc.platform.eq_ignore_ascii_case(target_platform_str))
        .collect();
    if matching.is_empty() {
        return None;
    }

    // Check for existing credentials
    let maybe_plat = Platform::from_str(target_platform_str).ok();
    if let Some(plat) = maybe_plat {
        let creds = match bot_api.list_credentials(Some(plat.clone())).await {
            Ok(c) => c,
            Err(e) => {
                return Some(format!("Error checking credentials => {e}"));
            }
        };
        if !creds.is_empty() {
            let mut msg = String::new();
            msg.push_str(&format!(
                "Cannot remove '{target_platform_str}' because these accounts still exist:\n"
            ));
            for c in creds {
                let name = match bot_api.get_user(c.user_id).await {
                    Ok(Some(u)) => u.global_username.unwrap_or_else(|| c.user_id.to_string()),
                    _ => c.user_id.to_string(),
                };
                msg.push_str(&format!(
                    " - user='{name}' platform={:?} credential_id={}\n",
                    c.platform, c.credential_id
                ));
            }
            msg.push_str("All accounts must be removed before the platform can be deleted.\n");
            return Some(msg);
        }
    }

    println!("\nExisting config(s) for '{target_platform_str}':");
    for pc in matching.iter() {
        println!(
            " - id={} platform={} client_id={}",
            pc.platform_config_id,
            pc.platform,
            pc.client_id.as_deref().unwrap_or("NONE")
        );
    }
    println!("Enter the platform_config_id to remove (or leave blank to skip): ");
    print!("> ");
    let _ = stdout().flush();

    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let chosen_id = line.trim().to_string();
    if chosen_id.is_empty() {
        return Some(format!("Skipped removal for '{target_platform_str}'."));
    }

    match bot_api.remove_platform_config(&chosen_id).await {
        Ok(_) => Some(format!("Removed platform config with id={chosen_id}.")),
        Err(e) => Some(format!("Error removing => {e}")),
    }
}

async fn platform_show(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    let confs = match bot_api.list_platform_configs(Some(&platform_str)).await {
        Ok(list) => list,
        Err(e) => return format!("Error => {e}"),
    };
    if confs.is_empty() {
        return format!("No platform config found for '{platform_str}'.");
    }
    let pc = &confs[0];
    let mut out = String::new();
    out.push_str(&format!("platform={} (id={})\n", pc.platform, pc.platform_config_id));
    out.push_str(&format!("client_id='{}'\n", pc.client_id.as_deref().unwrap_or("NONE")));
    out.push_str(&format!("client_secret='{}'\n", pc.client_secret.as_deref().unwrap_or("NONE")));
    out
}