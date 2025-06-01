// File: maowbot-tui/src/commands/command.rs
//! Allows editing built-in or custom commands. Example usage in the TUI:
//!   "command list [platform]"
//!   "command setcooldown <commandName> <seconds> [platform]"
//!   "command setwarnonce <commandName> <true|false> [platform]"
//!   "command setrespond <commandName> <accountOrNone> [platform]"
//!   "command setplatform <commandName> <newPlatform> [oldPlatform]"
//!   "command enable <commandName> [platform]"
//!   "command disable <commandName> [platform]"

use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use maowbot_common::models::Command;
use maowbot_common::traits::api::BotApi;
use maowbot_core::Error;

/// Entry point from TUI: "command <subcmd> <args...>"
pub async fn handle_command_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: command <list|setcooldown|setwarnonce|setrespond|setplatform|enable|disable> [args...]".to_string();
    }
    match args[0].to_lowercase().as_str() {
        "list" => {
            // Behavior change:
            // If the user typed "command list" with no further args, list commands for *all* platforms.
            // If "command list vrchat" is typed, list only vrchat, etc.
            if args.len() == 1 {
                // No platform specified => gather from all known platforms
                // You might keep a list of known platforms or have a dedicated "list_all_commands" function
                let known_platforms = vec!["twitch-irc", "twitch", "vrchat", "discord", "twitch-eventsub"];
                let mut out = String::new();
                for plat in &known_platforms {
                    match bot_api.list_commands(plat).await {
                        Ok(cmds) if !cmds.is_empty() => {
                            out.push_str(&format!("Commands for platform '{}':\n", plat));
                            for c in cmds {
                                out.push_str(&format!(
                                    " - {} (id={}) active={} cd={}s warnonce={} respond={:?}\n",
                                    c.command_name,
                                    c.command_id,
                                    c.is_active,
                                    c.cooldown_seconds,
                                    c.cooldown_warnonce,
                                    c.respond_with_credential
                                ));
                            }
                            out.push('\n');
                        }
                        _ => { /* skip if empty or error */ }
                    }
                }
                if out.trim().is_empty() {
                    out = "No commands found for any platform.".to_string();
                }
                out
            } else {
                // "command list <platform>"
                let plat = args[1];
                match bot_api.list_commands(plat).await {
                    Ok(cmds) => {
                        if cmds.is_empty() {
                            format!("No commands found for platform '{}'.", plat)
                        } else {
                            let mut out = format!("Commands for platform '{}':\n", plat);
                            for c in cmds {
                                out.push_str(&format!(
                                    " - {} (id={}) active={} cd={}s warnonce={} respond={:?}\n",
                                    c.command_name,
                                    c.command_id,
                                    c.is_active,
                                    c.cooldown_seconds,
                                    c.cooldown_warnonce,
                                    c.respond_with_credential
                                ));
                            }
                            out
                        }
                    }
                    Err(e) => format!("Error listing commands => {e}"),
                }
            }
        }

        "setcooldown" => {
            // "command setcooldown <commandName> <seconds> [platform]"
            if args.len() < 3 {
                return "Usage: command setcooldown <commandName> <seconds> [platform]".to_string();
            }
            let command_name = args[1];
            let sec_str = args[2];
            let platform = args.get(3).map(|s| *s).unwrap_or("twitch-irc");
            let secs = match sec_str.parse::<i32>() {
                Ok(n) => n,
                Err(_) => return "Cooldown seconds must be an integer.".to_string(),
            };
            match set_cooldown(bot_api, platform, command_name, secs).await {
                Ok(_) => format!("Cooldown set to {}s for command '{}'.", secs, command_name),
                Err(e) => format!("Error => {e}"),
            }
        }

        "setwarnonce" => {
            // "command setwarnonce <commandName> <true|false> [platform]"
            if args.len() < 3 {
                return "Usage: command setwarnonce <commandName> <true|false> [platform]".to_string();
            }
            let command_name = args[1];
            let tf_str = args[2].to_lowercase();
            let platform = args.get(3).map(|s| *s).unwrap_or("twitch-irc");
            let tf = match tf_str.as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                _ => return "Please specify true or false.".to_string(),
            };
            match set_warnonce(bot_api, platform, command_name, tf).await {
                Ok(_) => format!("warnonce set to {} for '{}'.", tf, command_name),
                Err(e) => format!("Error => {e}"),
            }
        }

        "setrespond" => {
            // "command setrespond <commandName> <credentialId|username|none> [platform]"
            if args.len() < 3 {
                return "Usage: command setrespond <commandName> <credentialId|username|none> [platform]".to_string();
            }
            let command_name = args[1];
            let account_arg = args[2];
            let platform = args.get(3).map(|s| *s).unwrap_or("");
            match set_respond_with(bot_api, command_name, account_arg, platform).await {
                Ok(_) => format!("Responding credential updated for '{}'.", command_name),
                Err(e) => format!("Error => {e}"),
            }
        }

        "setplatform" => {
            // "command setplatform <commandName> <newPlatform> [oldPlatform]"
            // Example usage:
            //   command setplatform !test vrchat twitch-irc
            if args.len() < 3 {
                return "Usage: command setplatform <commandName> <newPlatform> [oldPlatform]".to_string();
            }
            let command_name = args[1];
            let new_platform = args[2];
            let old_platform = args.get(3).map(|s| *s).unwrap_or("twitch-irc");
            match set_platform(bot_api, old_platform, command_name, new_platform).await {
                Ok(_) => format!(
                    "Platform changed for command '{}' from '{}' to '{}'.",
                    command_name, old_platform, new_platform
                ),
                Err(e) => format!("Error => {e}"),
            }
        }

        "enable" => {
            // "command enable <commandName> [platform]"
            if args.len() < 2 {
                return "Usage: command enable <commandName> [platform]".to_string();
            }
            let command_name = args[1];
            let platform = args.get(2).map(|s| *s).unwrap_or("twitch-irc");
            match set_active(bot_api, platform, command_name, true).await {
                Ok(_) => format!("Enabled command '{}'.", command_name),
                Err(e) => format!("Error => {e}"),
            }
        }

        "disable" => {
            if args.len() < 2 {
                return "Usage: command disable <commandName> [platform]".to_string();
            }
            let command_name = args[1];
            let platform = args.get(2).map(|s| *s).unwrap_or("twitch-irc");
            match set_active(bot_api, platform, command_name, false).await {
                Ok(_) => format!("Disabled command '{}'.", command_name),
                Err(e) => format!("Error => {e}"),
            }
        }

        _ => {
            "Unknown subcommand. Usage: command <list|setcooldown|setwarnonce|setrespond|setplatform|enable|disable> [args...]".to_string()
        }
    }
}

async fn set_cooldown(
    bot_api: &Arc<dyn BotApi>,
    platform: &str,
    cmd_name: &str,
    secs: i32,
) -> Result<(), Error> {
    let mut cmd = get_command_by_name(bot_api, platform, cmd_name).await?;
    cmd.cooldown_seconds = secs;
    cmd.updated_at = Utc::now();
    bot_api.update_command(&cmd).await
}

async fn set_warnonce(
    bot_api: &Arc<dyn BotApi>,
    platform: &str,
    cmd_name: &str,
    value: bool,
) -> Result<(), Error> {
    let mut cmd = get_command_by_name(bot_api, platform, cmd_name).await?;
    cmd.cooldown_warnonce = value;
    cmd.updated_at = Utc::now();
    bot_api.update_command(&cmd).await
}

/// Main function to set the `respond_with_credential` field on a command.
async fn set_respond_with(
    bot_api: &Arc<dyn BotApi>,
    cmd_name: &str,
    account_arg: &str,
    maybe_platform: &str,
) -> Result<(), Error> {
    // 1) find the command
    let default_plat = if maybe_platform.is_empty() { "twitch-irc" } else { maybe_platform };
    let mut cmd = get_command_by_name(bot_api, default_plat, cmd_name).await?;

    // 2) handle "none"
    if account_arg.eq_ignore_ascii_case("none") {
        cmd.respond_with_credential = None;
        cmd.updated_at = chrono::Utc::now();
        bot_api.update_command(&cmd).await?;
        return Ok(());
    }

    // 3) try parse as UUID
    if let Ok(parsed_id) = Uuid::parse_str(account_arg) {
        // We'll do a quick check if that credential actually exists.
        let creds = bot_api.list_credentials(None).await?;
        let found = creds.iter().any(|c| c.credential_id == parsed_id);
        if !found {
            return Err(Error::Database(sqlx::Error::RowNotFound));
        }
        // Set it:
        cmd.respond_with_credential = Some(parsed_id);
        cmd.updated_at = chrono::Utc::now();
        bot_api.update_command(&cmd).await?;
        return Ok(());
    }

    // 4) Otherwise, treat `account_arg` as a global_username
    let user = match bot_api.find_user_by_name(account_arg).await {
        Ok(u) => u,
        Err(e) => {
            return Err(Error::Platform(format!("No user with name='{}': {e}", account_arg)));
        }
    };

    // Now find that user’s credentials for the platform that the command uses.
    let cmd_platform = &cmd.platform; // e.g. "twitch-irc"
    let user_creds = bot_api.list_credentials(None).await?;
    let matches = user_creds
        .into_iter()
        .filter(|c| {
            c.user_id == user.user_id
                && c.platform.to_string().eq_ignore_ascii_case(cmd_platform)
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return Err(Error::Platform(format!(
            "User '{}' has no credentials for platform '{}'.",
            account_arg, cmd_platform
        )));
    }
    if matches.len() > 1 {
        return Err(Error::Platform(format!(
            "User '{}' has multiple credentials for '{}'; please specify a UUID.",
            account_arg, cmd_platform
        )));
    }

    let cred_id = matches[0].credential_id;
    cmd.respond_with_credential = Some(cred_id);
    cmd.updated_at = chrono::Utc::now();
    bot_api.update_command(&cmd).await?;
    Ok(())
}

/// Helper: change the `platform` field of an existing command.
async fn set_platform(
    bot_api: &Arc<dyn BotApi>,
    old_platform: &str,
    cmd_name: &str,
    new_platform: &str,
) -> Result<(), Error> {
    // 1) Get the command from the old platform
    let mut cmd = get_command_by_name(bot_api, old_platform, cmd_name).await?;

    // 2) Set to new platform and update
    cmd.platform = new_platform.to_string();
    cmd.updated_at = chrono::Utc::now();
    bot_api.update_command(&cmd).await?;
    Ok(())
}

/// Helper to retrieve a command by name from a given platform.
async fn get_command_by_name(
    bot_api: &Arc<dyn BotApi>,
    platform: &str,
    cmd_name: &str
) -> Result<Command, Error> {
    let all = bot_api.list_commands(platform).await?;
    let lowered = if cmd_name.starts_with('!') {
        cmd_name[1..].to_lowercase()
    } else {
        cmd_name.to_lowercase()
    };

    let found = all.into_iter().find(|c| {
        let c_lower = c.command_name.to_lowercase();
        c_lower == format!("!{}", lowered) || c_lower == lowered
    });

    if let Some(c) = found {
        Ok(c)
    } else {
        Err(Error::Platform(format!("Command '{}' not found on platform '{}'.", cmd_name, platform)))
    }
}

async fn set_active(
    bot_api: &Arc<dyn BotApi>,
    platform: &str,
    cmd_name: &str,
    active: bool
) -> Result<(), Error> {
    let mut cmd = get_command_by_name(bot_api, platform, cmd_name).await?;
    cmd.is_active = active;
    cmd.updated_at = Utc::now();
    bot_api.update_command(&cmd).await
}
