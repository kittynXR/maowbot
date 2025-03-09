use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use maowbot_core::Error;
use maowbot_core::models::Redeem;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::plugins::bot_api::redeem_api::RedeemApi;
use serde::Deserialize;

/// A small struct matching your `AutostartConfig` shape
#[derive(Debug, Deserialize)]
struct AutostartConfig {
    pub accounts: Vec<(String, String)>,
}

/// The main entry point from the TUI dispatcher:
pub async fn handle_redeem_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: redeem <list|enable|pause|offline|setcost|setprompt|setplugin|setcommand|remove|sync>".to_string();
    }

    match args[0].to_lowercase().as_str() {
        // -----------------------------------------------------
        // LIST
        // -----------------------------------------------------
        "list" => {
            match bot_api.list_redeems("twitch-eventsub").await {
                Ok(list) => {
                    if list.is_empty() {
                        return "No redeems found for 'twitch-eventsub'.".to_string();
                    }

                    // Partition: broadcaster-managed = is_managed=false, bot-managed = is_managed=true
                    let (broadcaster, bot_managed): (Vec<_>, Vec<_>) =
                        list.into_iter().partition(|rd| rd.is_managed == false);

                    // Attempt to parse associated Twitch accounts from "autostart" config
                    let mut all_accounts = Vec::new();
                    if let Ok(Some(config_str)) = bot_api.get_bot_config_value("autostart").await {
                        if let Ok(parsed) = serde_json::from_str::<AutostartConfig>(&config_str) {
                            for (plat, acct) in parsed.accounts {
                                if plat.eq_ignore_ascii_case("twitch") || plat.eq_ignore_ascii_case("twitch-irc") {
                                    all_accounts.push(acct);
                                }
                            }
                        }
                    }
                    let accounts_str = if all_accounts.is_empty() {
                        "[No associated Twitch accounts found in autostart]".to_string()
                    } else {
                        format!("Associated Twitch accounts: {}", all_accounts.join(", "))
                    };

                    // Build the final output
                    let mut output = String::new();
                    output.push_str("Current Redeems (twitch-eventsub):\n\n");

                    // Print broadcaster-managed section
                    if broadcaster.is_empty() {
                        output.push_str("[Broadcaster-managed redeems]\n(no items)\n\n");
                    } else {
                        output.push_str("[Broadcaster-managed redeems]\n");
                        output.push_str(&format_table(&broadcaster));
                        output.push_str("\n");
                    }

                    // Print bot-managed section
                    if bot_managed.is_empty() {
                        output.push_str("[Bot-managed redeems]\n(no items)\n\n");
                    } else {
                        output.push_str("[Bot-managed redeems]\n");
                        output.push_str(&format_table(&bot_managed));
                        output.push_str("\n");
                    }

                    output.push_str(&accounts_str);
                    output
                }
                Err(e) => format!("Error listing redeems => {e}"),
            }
        }

        // -----------------------------------------------------
        // ENABLE
        // -----------------------------------------------------
        "enable" => {
            if args.len() < 2 {
                return "Usage: redeem enable <redeemName>".to_string();
            }
            let name = args[1];
            match get_redeem_by_name(bot_api, name).await {
                Ok(redeem) => {
                    if let Err(e) = bot_api.set_redeem_active(redeem.redeem_id, true).await {
                        return format!("Error enabling => {e}");
                    }
                    format!("Redeem '{}' is now enabled.", redeem.reward_name)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        // -----------------------------------------------------
        // PAUSE (set is_active=false)
        // -----------------------------------------------------
        "pause" => {
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

        // -----------------------------------------------------
        // OFFLINE
        // Toggles `active_offline` boolean in DB.
        // -----------------------------------------------------
        "offline" => {
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

        // -----------------------------------------------------
        // SETCOST
        // -----------------------------------------------------
        "setcost" => {
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

        // -----------------------------------------------------
        // SETPROMPT
        // (demo only, not stored in DB)
        // -----------------------------------------------------
        "setprompt" => {
            if args.len() < 3 {
                return "Usage: redeem setprompt <promptText> <redeemName>".to_string();
            }
            let prompt_text = args[1];
            let name = args[2];
            match get_redeem_by_name(bot_api, name).await {
                Ok(r) => {
                    format!("(Demo) Would set prompt for '{}' => '{}'", r.reward_name, prompt_text)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        // -----------------------------------------------------
        // SETPLUGIN
        // -----------------------------------------------------
        "setplugin" => {
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
                        Ok(_) => format!(
                            "Redeem '{}' plugin_name set to '{}'.",
                            r.reward_name, plugin_name
                        ),
                        Err(e) => format!("Error updating => {e}"),
                    }
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        // -----------------------------------------------------
        // SETCOMMAND
        // -----------------------------------------------------
        "setcommand" => {
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
                        Ok(_) => format!(
                            "Redeem '{}' command_name set to '{}'.",
                            r.reward_name, command_name
                        ),
                        Err(e) => format!("Error updating => {e}"),
                    }
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        // -----------------------------------------------------
        // REMOVE
        // -----------------------------------------------------
        "remove" => {
            if args.len() < 2 {
                return "Usage: redeem remove <redeemName>".to_string();
            }
            let name = args[1];
            match get_redeem_by_name(bot_api, name).await {
                Ok(r) => {
                    if let Err(e) = bot_api.delete_redeem(r.redeem_id).await {
                        return format!("Error removing => {e}");
                    }
                    format!("Redeem '{}' removed from DB.", r.reward_name)
                }
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        // -----------------------------------------------------
        // SYNC
        // -----------------------------------------------------
        "sync" => {
            match bot_api.sync_redeems().await {
                Ok(_) => "Redeem sync task started successfully.".to_string(),
                Err(e) => format!("Error calling redeem sync => {e}"),
            }
        }

        // -----------------------------------------------------
        // UNKNOWN
        // -----------------------------------------------------
        _ => {
            "Unknown redeem subcommand. Type 'help redeem' for usage.".to_string()
        }
    }
}

/// Utility: find a Redeem by `reward_name` (case-insensitive) among all
/// 'twitch-eventsub' redeems in the DB.
async fn get_redeem_by_name(bot_api: &Arc<dyn BotApi>, name: &str) -> Result<Redeem, Error> {
    let all = bot_api.list_redeems("twitch-eventsub").await?;
    let lowered = name.to_lowercase();

    for rd in all {
        if rd.reward_name.to_lowercase() == lowered {
            return Ok(rd);
        }
    }
    Err(Error::Platform(format!("No redeem found matching reward_name='{name}'")))
}

/// Helper for pretty-printing a table of Redeems.
fn format_table(redeems: &[Redeem]) -> String {
    if redeems.is_empty() {
        return "(none)\n".to_string();
    }

    // Collect the data rows:
    //  1) Name
    //  2) Cost
    //  3) Actv (is_active)
    //  4) Offl (active_offline)
    //  5) Plugin
    //  6) Command
    let mut rows: Vec<[String; 6]> = Vec::new();
    for rd in redeems {
        let plugin_s = rd.plugin_name.clone().unwrap_or("-".to_string());
        let cmd_s = rd.command_name.clone().unwrap_or("-".to_string());
        rows.push([
            rd.reward_name.clone(),
            format!("{}", rd.cost),
            format!("{}", rd.is_active),
            format!("{}", rd.active_offline),
            plugin_s,
            cmd_s,
        ]);
    }

    // Determine column widths
    let mut col_widths = [0usize; 6];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(cell.len());
        }
    }
    // Add extra spacing for readability
    for w in &mut col_widths {
        *w += 2;
    }

    let headers = ["Name", "Cost", "Actv", "Offl", "Plugin", "Command"];
    let mut out = String::new();

    // Format header row with a delimiter space between columns.
    for i in 0..headers.len() {
        let head = headers[i];
        let formatted = if i == 1 {
            let pad = col_widths[i].saturating_sub(head.len());
            format!("{}{}", " ".repeat(pad), head)
        } else {
            let pad = col_widths[i].saturating_sub(head.len());
            format!("{}{}", head, " ".repeat(pad))
        };
        out.push_str(&formatted);
        if i < headers.len() - 1 {
            out.push_str(" "); // delimiter space between columns
        }
    }
    out.push('\n');

    // Format each data row with delimiter space between columns.
    for row in rows {
        for i in 0..row.len() {
            let cell = &row[i];
            let formatted = if i == 1 {
                let pad = col_widths[i].saturating_sub(cell.len());
                format!("{}{}", " ".repeat(pad), cell)
            } else {
                let pad = col_widths[i].saturating_sub(cell.len());
                format!("{}{}", cell, " ".repeat(pad))
            };
            out.push_str(&formatted);
            if i < row.len() - 1 {
                out.push_str(" ");
            }
        }
        out.push('\n');
    }

    out
}
