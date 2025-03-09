// File: maowbot-tui/src/commands/redeem.rs

use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use maowbot_core::Error;
use maowbot_core::models::Redeem;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::plugins::bot_api::redeem_api::RedeemApi;
use serde::Deserialize;

/// Matches your 'autostart' config structure, if used for listing associated accounts.
#[derive(Debug, Deserialize)]
struct AutostartConfig {
    pub accounts: Vec<(String, String)>,
}

/// Main entry point from the TUI dispatcher. Now includes the new "redeem add" subcommand.
pub async fn handle_redeem_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: redeem <list|info|add|enable|pause|offline|setcost|setprompt|setplugin|setcommand|remove|sync>".to_string();
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

                    // Partition: broadcaster-managed (is_managed=false) vs. bot-managed (is_managed=true)
                    let (broadcaster, bot_managed): (Vec<_>, Vec<_>) =
                        list.into_iter().partition(|rd| rd.is_managed == false);

                    // Try to parse associated Twitch accounts from "autostart"
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

                    // Build final output
                    let mut output = String::new();
                    output.push_str("Current Redeems (twitch-eventsub):\n\n");

                    // Broadcaster-managed
                    if broadcaster.is_empty() {
                        output.push_str("[Broadcaster-managed redeems]\n(no items)\n\n");
                    } else {
                        output.push_str("[Broadcaster-managed redeems]\n");
                        output.push_str(&format_table(&broadcaster));
                        output.push_str("\n");
                    }

                    // Bot-managed
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
        // INFO
        // -----------------------------------------------------
        "info" => {
            if args.len() < 2 {
                return "Usage: redeem info <redeemName>".to_string();
            }
            let name = args[1];
            match get_redeem_by_name(bot_api, name).await {
                Ok(redeem) => format_redeem_details(&redeem),
                Err(e) => format!("Could not find redeem '{name}' => {e}"),
            }
        }

        // -----------------------------------------------------
        // ADD (NEW SUBCOMMAND)
        // -----------------------------------------------------
        "add" => {
            // Example usage:
            //   redeem add <rewardName> <cost> [--managed] [--offline] [--dynamic]
            if args.len() < 3 {
                return "Usage: redeem add <rewardName> <cost> [--managed] [--offline] [--dynamic]".to_string();
            }

            let reward_name = args[1];
            let cost_str = args[2];
            let cost = match cost_str.parse::<i32>() {
                Ok(n) => n,
                Err(_) => return "Cost must be an integer.".to_string(),
            };

            // Parse optional flags
            let mut is_managed = false;
            let mut active_offline = false;
            let mut dynamic_pricing = false;
            for flag in &args[3..] {
                match flag.to_lowercase().as_str() {
                    "--managed" => is_managed = true,
                    "--offline" => active_offline = true,
                    "--dynamic" => dynamic_pricing = true,
                    other => {
                        return format!("Unknown flag '{other}'. Valid flags: --managed, --offline, --dynamic");
                    }
                }
            }

            // 1) Insert new row. We'll pass empty "" for reward_id; the DB sets is_managed=false by default.
            match bot_api.create_redeem("twitch-eventsub", "", reward_name, cost, dynamic_pricing).await {
                Ok(mut new_rd) => {
                    // 2) If user wants is_managed or offline, update the newly created row
                    if is_managed || active_offline {
                        new_rd.is_managed = is_managed;
                        new_rd.active_offline = active_offline;
                        new_rd.updated_at = Utc::now();

                        if let Err(e) = bot_api.update_redeem(&new_rd).await {
                            return format!("(Partial) Created redeem but error updating is_managed/offline => {e}");
                        }
                    }

                    format!(
                        "New redeem '{}' created with cost={} (managed={}, offline={}, dynamic={}).\n\
                         You can run 'redeem sync' or restart to push it to Twitch if managed=true.",
                        new_rd.reward_name, new_rd.cost, is_managed, active_offline, dynamic_pricing
                    )
                }
                Err(e) => format!("Error creating => {e}"),
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

/// Looks up a Redeem by reward_name (case-insensitive) among all
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

/// Helper to pretty-print a table of Redeems (for `redeem list`).
fn format_table(redeems: &[Redeem]) -> String {
    if redeems.is_empty() {
        return "(none)\n".to_string();
    }

    // 1) Collect data: [Name, Cost, Actv, Offl, Plugin, Command]
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

    // 2) Determine column widths
    let mut col_widths = [0usize; 6];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(cell.len());
        }
    }
    // Add a bit of padding for readability
    for w in &mut col_widths {
        *w += 2;
    }

    let headers = ["Name", "Cost", "Actv", "Offl", "Plugin", "Command"];
    let mut out = String::new();

    // 3) Format header row
    for i in 0..headers.len() {
        let head = headers[i];
        let pad = col_widths[i].saturating_sub(head.len());
        if i == 1 {
            // Right-align cost
            out.push_str(&format!("{}{}", " ".repeat(pad), head));
        } else {
            out.push_str(&format!("{}{}", head, " ".repeat(pad)));
        }
        if i < headers.len() - 1 {
            out.push_str(" ");
        }
    }
    out.push('\n');

    // 4) Format data rows
    for row in rows {
        for i in 0..row.len() {
            let cell = &row[i];
            let pad = col_widths[i].saturating_sub(cell.len());
            if i == 1 {
                out.push_str(&format!("{}{}", " ".repeat(pad), cell));
            } else {
                out.push_str(&format!("{}{}", cell, " ".repeat(pad)));
            }
            if i < row.len() - 1 {
                out.push_str(" ");
            }
        }
        out.push('\n');
    }

    out
}

/// Helper function to format all the details of a single Redeem (for `redeem info`).
fn format_redeem_details(rd: &Redeem) -> String {
    format!(
        "Redeem Info\n\
         -------------\n\
         redeem_id:      {}\n\
         platform:       {}\n\
         reward_id:      {}\n\
         reward_name:    {}\n\
         cost:           {}\n\
         is_active:      {}\n\
         dynamic_pricing: {}\n\
         created_at:     {}\n\
         updated_at:     {}\n\
         active_offline: {}\n\
         is_managed:     {}\n\
         plugin_name:    {}\n\
         command_name:   {}\n",
        rd.redeem_id,
        rd.platform,
        rd.reward_id,
        rd.reward_name,
        rd.cost,
        rd.is_active,
        rd.dynamic_pricing,
        rd.created_at,
        rd.updated_at,
        rd.active_offline,
        rd.is_managed,
        rd.plugin_name.as_deref().unwrap_or("-"),
        rd.command_name.as_deref().unwrap_or("-"),
    )
}
