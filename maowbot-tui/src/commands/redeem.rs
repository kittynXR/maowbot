// File: maowbot-tui/src/commands/redeem.rs
//
// Implements the "redeem" command group in the TUI. Example usage:
//
//   redeem list
//   redeem enable <redeemName>
//   redeem pause <redeemName>
//   redeem offline <redeemName>
//   redeem setcost <points> <redeemName>
//   redeem setprompt <promptText> <redeemName>   [not fully persisted in DB, demonstration only]
//   redeem setplugin <pluginName> <redeemName>
//   redeem setcommand <commandName> <redeemName>
//   redeem setcooldown <seconds> <redeemName>    [not implemented in DB, demonstration only]
//   redeem setaccount <accountName> <redeemName> [placeholder demonstration]
//   redeem remove <accountName> <redeemName>

use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;

use maowbot_core::Error;
use maowbot_core::models::Redeem;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::plugins::bot_api::redeem_api::RedeemApi;

/// The main entry point from the TUI dispatcher:
pub async fn handle_redeem_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: redeem <list|enable|pause|offline|setcost|setprompt|setplugin|setcommand|setcooldown|setaccount|remove> ...".to_string();
    }

    match args[0].to_lowercase().as_str() {
        "list" => {
            match bot_api.list_redeems("twitch-eventsub").await {
                Ok(list) => {
                    if list.is_empty() {
                        "No redeems found for 'twitch-eventsub'.".to_string()
                    } else {
                        let mut out = String::from("Current Redeems (twitch-eventsub):\n");
                        for r in list {
                            out.push_str(&format!(
                                " - name='{}' cost={} active={} offline={} plugin={:?} command={:?}\n",
                                r.reward_name, r.cost, r.is_active, r.active_offline, r.plugin_name, r.command_name
                            ));
                        }
                        out
                    }
                }
                Err(e) => format!("Error listing redeems => {e}"),
            }
        }

        "enable" => {
            if args.len() < 2 {
                return "Usage: redeem enable <redeemName>".to_string();
            }
            let name = args[1];
            match get_redeem_by_name(bot_api, name).await {
                Ok(mut redeem) => {
                    // We'll use the set_redeem_active call:
                    if let Err(e) = bot_api.set_redeem_active(redeem.redeem_id, true).await {
                        return format!("Error enabling => {e}");
                    }
                    format!("Redeem '{}' is now enabled.", redeem.reward_name)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "pause" => {
            // We'll treat "pause" as setting is_active=false
            if args.len() < 2 {
                return "Usage: redeem pause <redeemName>".to_string();
            }
            let name = args[1];
            match get_redeem_by_name(bot_api, name).await {
                Ok(redeem) => {
                    if let Err(e) = bot_api.set_redeem_active(redeem.redeem_id, false).await {
                        return format!("Error pausing => {e}");
                    }
                    format!("Redeem '{}' has been paused (is_active=false).", redeem.reward_name)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "offline" => {
            // "offline <redeemName>" toggles `active_offline`
            if args.len() < 2 {
                return "Usage: redeem offline <redeemName>".to_string();
            }
            let name = args[1];
            match get_redeem_by_name(bot_api, name).await {
                Ok(mut redeem) => {
                    let new_val = !redeem.active_offline;
                    redeem.active_offline = new_val;
                    redeem.updated_at = Utc::now();
                    let update_result = bot_api.update_redeem(&redeem).await;
                    match update_result {
                        Ok(_) => format!(
                            "Redeem '{}' offline-availability toggled to {}.",
                            redeem.reward_name, redeem.active_offline
                        ),
                        Err(e) => format!("Error updating => {e}"),
                    }
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "setcost" => {
            // "setcost <points> <redeemName>"
            if args.len() < 3 {
                return "Usage: redeem setcost <points> <redeemName>".to_string();
            }
            let cost_str = args[1];
            let name = args[2];
            let cost = match cost_str.parse::<i32>() {
                Ok(n) => n,
                Err(_) => return "Cost must be an integer.".to_string(),
            };
            match get_redeem_by_name(bot_api, name).await {
                Ok(redeem) => {
                    if let Err(e) = bot_api.update_redeem_cost(redeem.redeem_id, cost).await {
                        return format!("Error setting cost => {e}");
                    }
                    format!("Redeem '{}' cost set to {}.", redeem.reward_name, cost)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "setprompt" => {
            // "setprompt <promptText> <redeemName>"
            // The DB struct currently has no dedicated 'prompt' field.
            // We'll demonstrate a placeholder approach only (not fully stored).
            if args.len() < 3 {
                return "Usage: redeem setprompt <promptText> <redeemName>".to_string();
            }
            let prompt_text = args[1];
            let name = args[2];
            match get_redeem_by_name(bot_api, name).await {
                Ok(r) => {
                    // We'll pretend we store it or do something. The actual DB field doesn't exist.
                    format!("(Demo) Would set prompt for '{}' => '{}'", r.reward_name, prompt_text)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "setplugin" => {
            // "setplugin <pluginName> <redeemName>"
            if args.len() < 3 {
                return "Usage: redeem setplugin <pluginName> <redeemName>".to_string();
            }
            let plugin_name = args[1];
            let name = args[2];
            match get_redeem_by_name(bot_api, name).await {
                Ok(mut r) => {
                    r.plugin_name = Some(plugin_name.to_string());
                    r.updated_at = Utc::now();
                    match bot_api.update_redeem(&r).await {
                        Ok(_) => format!("Redeem '{}' plugin_name set to '{}'.", r.reward_name, plugin_name),
                        Err(e) => format!("Error updating => {e}"),
                    }
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "setcommand" => {
            // "setcommand <commandName> <redeemName>"
            if args.len() < 3 {
                return "Usage: redeem setcommand <commandName> <redeemName>".to_string();
            }
            let command_name = args[1];
            let name = args[2];
            match get_redeem_by_name(bot_api, name).await {
                Ok(mut r) => {
                    r.command_name = Some(command_name.to_string());
                    r.updated_at = Utc::now();
                    match bot_api.update_redeem(&r).await {
                        Ok(_) => {
                            format!("Redeem '{}' command_name set to '{}'.", r.reward_name, command_name)
                        }
                        Err(e) => format!("Error updating => {e}"),
                    }
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "setcooldown" => {
            // "setcooldown <seconds> <redeemName>"
            // The Redeem struct in DB does not have a cooldown field. We'll do a placeholder message.
            if args.len() < 3 {
                return "Usage: redeem setcooldown <seconds> <redeemName>".to_string();
            }
            let seconds = args[1];
            let name = args[2];
            match get_redeem_by_name(bot_api, name).await {
                Ok(r) => {
                    format!(
                        "(Demo) Would set cooldown={} for redeem '{}'. [Not stored in DB]",
                        seconds, r.reward_name
                    )
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "setaccount" => {
            // "redeem setaccount <accountName> <redeemName>"
            // In an actual multi-account scenario, you might store a different row or update the platform field.
            // We'll do a placeholder demonstration that is not persisted.
            if args.len() < 3 {
                return "Usage: redeem setaccount <accountName> <redeemName>".to_string();
            }
            let account_name = args[1];
            let name = args[2];
            match get_redeem_by_name(bot_api, name).await {
                Ok(r) => {
                    format!("(Demo) Would link redeem '{}' to account '{}'. [Not fully implemented]", r.reward_name, account_name)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        "remove" => {
            // "redeem remove <accountName> <redeemName>"
            // Possibly means removing from DB or from that specific account.
            // We'll demonstrate deleting the redeem entirely from DB.
            if args.len() < 3 {
                return "Usage: redeem remove <accountName> <redeemName>".to_string();
            }
            let account_name = args[1]; // currently unused
            let name = args[2];
            match get_redeem_by_name(bot_api, name).await {
                Ok(r) => {
                    if let Err(e) = bot_api.delete_redeem(r.redeem_id).await {
                        return format!("Error removing => {e}");
                    }
                    format!("Redeem '{}' removed from DB. (Requested by account '{}')", r.reward_name, account_name)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        _ => {
            "Unknown redeem subcommand. Type 'help redeem' for usage.".to_string()
        }
    }
}

/// Utility: find a Redeem by `reward_name` (case-insensitive) among all known "twitch-eventsub" redeems.
async fn get_redeem_by_name(bot_api: &Arc<dyn BotApi>, name: &str) -> Result<Redeem, Error> {
    let all = bot_api.list_redeems("twitch-eventsub").await?;
    let lowered = name.to_lowercase();

    // Some redeems might have slightly different display vs. internal. We'll match on `reward_name` ignoring case.
    for rd in all {
        if rd.reward_name.to_lowercase() == lowered {
            return Ok(rd);
        }
    }
    Err(Error::Platform(format!("No redeem found matching reward_name='{}'", name)))
}
