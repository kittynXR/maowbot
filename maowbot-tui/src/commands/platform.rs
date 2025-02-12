use std::sync::Arc;
use std::io::{Write, stdin, stdout};
use std::str::FromStr;
use maowbot_core::models::Platform;
use maowbot_core::plugins::bot_api::BotApi;
use tokio::runtime::Handle;

pub fn handle_platform_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: platform <add|remove|list|show> ...".to_string();
    }

    match args[0] {
        "add" => {
            if args.len() < 2 {
                return "Usage: platform add <platformName>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(plat) => handle_platform_add(plat, bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "remove" => {
            if args.len() < 2 {
                return "Usage: platform remove <platformName>".to_string();
            }
            match Platform::from_str(args[1]) {
                Ok(plat) => handle_platform_remove(plat, bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        "list" => {
            let maybe_platform = if args.len() > 1 { Some(args[1]) } else { None };
            let configs = Handle::current().block_on(bot_api.list_platform_configs(maybe_platform));
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
                Ok(plat) => platform_show(plat, bot_api),
                Err(_) => format!("Unknown platform '{}'", args[1]),
            }
        }
        _ => "Usage: platform <add|remove|list|show>".to_string(),
    }
}

fn handle_platform_add(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    println!("You are adding or updating the platform config for '{}'.", platform_str);

    // If this is "twitch", we will also handle "twitch-irc" and "twitch-eventsub"
    let also_add_irc_and_eventsub = matches!(plat, Platform::Twitch);

    let dev_console_url = match platform_str.as_str() {
        "twitch"      => Some("https://dev.twitch.tv/console"),
        "discord"     => Some("https://discord.com/developers/applications"),
        "vrchat"      => Some("https://dashboard.vrchat.com/"),
        "twitch-irc"  => None,
        "twitch-eventsub" => None,
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

    // Now store the main platform config:
    let result_main = Handle::current().block_on(async {
        bot_api.create_platform_config(plat.clone(), client_id.clone(), secret_opt.clone()).await
    });

    if let Err(e) = result_main {
        return format!("Error storing config for '{}': {:?}", platform_str, e);
    }

    // If "twitch", also do "twitch-irc" and "twitch-eventsub"
    if also_add_irc_and_eventsub {
        let result_irc = Handle::current().block_on(async {
            bot_api.create_platform_config(Platform::TwitchIRC, client_id.clone(), secret_opt.clone()).await
        });
        if let Err(e) = result_irc {
            println!("Warning: could not create 'twitch-irc' config => {:?}", e);
        }

        // If you have a separate enumerator for "twitch-eventsub", do that:
        // For example, if your `Platform` has a variant `Platform::TwitchEventSub`,
        // you might represent it internally as "twitch-eventsub" or similar:
        let result_eventsub = Handle::current().block_on(async {
            bot_api.create_platform_config(Platform::TwitchEventSub, client_id.clone(), secret_opt.clone()).await
        });
        if let Err(e) = result_eventsub {
            println!("Warning: could not create 'twitch-eventsub' config => {:?}", e);
        }
    }

    format!("Platform config upserted for '{}'.", platform_str)
}

fn handle_platform_remove(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    println!("Removing platform config(s) for '{}'.", platform_str);

    let also_remove_irc_and_eventsub = matches!(plat, Platform::Twitch);

    // First list all existing platform configs
    let list = match Handle::current().block_on(bot_api.list_platform_configs(None)) {
        Ok(lst) => lst,
        Err(e) => {
            return format!("Error listing platform configs => {:?}", e);
        }
    };
    if list.is_empty() {
        return "No platform configs found in the database.".to_string();
    }

    // We remove the config row(s) that match `plat`:
    let remove_main = remove_platform_config_by_name(&list, &platform_str, bot_api);
    // If `twitch`, also remove "twitch-irc" and "twitch-eventsub":
    if also_remove_irc_and_eventsub {
        let _ = remove_platform_config_by_name(&list, "twitch-irc", bot_api);
        let _ = remove_platform_config_by_name(&list, "twitch-eventsub", bot_api);
    }

    match remove_main {
        Some(msg) => msg,  // success or error
        None => format!("No platform config found for '{}'.", platform_str),
    }
}

/// Helper that looks in the provided list for a config whose `platform` matches `target_platform_str`,
/// then prompts the user to confirm removal, and calls `remove_platform_config(...)`.
fn remove_platform_config_by_name(
    list: &[maowbot_core::plugins::bot_api::PlatformConfigData],
    target_platform_str: &str,
    bot_api: &Arc<dyn BotApi>,
) -> Option<String> {
    // Find any matching row(s):
    let matching: Vec<_> = list
        .iter()
        .filter(|pc| pc.platform.eq_ignore_ascii_case(target_platform_str))
        .collect();

    if matching.is_empty() {
        return None; // no matching row
    }

    // If multiple, show them all (rare, but possible)
    println!("\nExisting config(s) for '{}':", target_platform_str);
    for pc in &matching {
        println!(
            " - id={} platform={} client_id={}",
            pc.platform_config_id,
            pc.platform,
            pc.client_id.as_deref().unwrap_or("NONE")
        );
    }

    // Prompt user which config_id to remove (or to remove them all)
    println!("Enter the platform_config_id to remove (or leave blank to skip): ");
    print!("> ");
    let _ = stdout().flush();

    let mut line = String::new();
    let _ = stdin().read_line(&mut line);
    let chosen_id = line.trim().to_string();
    if chosen_id.is_empty() {
        return Some(format!("Skipped removal for '{}'.", target_platform_str));
    }

    let rm_res = Handle::current().block_on(bot_api.remove_platform_config(&chosen_id));
    match rm_res {
        Ok(_) => Some(format!("Removed platform config with id={}.", chosen_id)),
        Err(e) => Some(format!("Error removing => {:?}", e)),
    }
}

fn platform_show(plat: Platform, bot_api: &Arc<dyn BotApi>) -> String {
    let platform_str = plat.to_string();
    let confs = Handle::current().block_on(bot_api.list_platform_configs(Some(&platform_str)));
    match confs {
        Ok(list) => {
            if list.is_empty() {
                return format!("No platform config found for '{}'.", platform_str);
            }
            let pc = &list[0];
            let mut out = String::new();
            out.push_str(&format!("platform={} (id={})\n", pc.platform, pc.platform_config_id));
            out.push_str(&format!("client_id='{}'\n", pc.client_id.as_deref().unwrap_or("NONE")));
            out.push_str(&format!("client_secret='{}'\n",
                                  pc.client_secret.as_deref().unwrap_or("NONE")));
            out
        }
        Err(e) => format!("Error => {:?}", e),
    }
}