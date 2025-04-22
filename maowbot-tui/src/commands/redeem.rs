use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use maowbot_common::models::Redeem;
use maowbot_common::traits::api::BotApi;

/// Matches your 'autostart' config structure, if used for listing associated accounts.
#[derive(Debug, Deserialize)]
struct AutostartConfig {
    pub accounts: Vec<(String, String)>,
}

/// Main entry point from the TUI dispatcher for "redeem" subcommands.
///
/// NOTE: We have renamed the output labels from “broadcaster-managed” to
/// “web-app managed” and from “bot-managed” to “internally managed.”
pub async fn handle_redeem_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage: redeem <list|info|add|enable|pause|offline|setcost|setprompt|setplugin|setcommand|setinput|remove|sync>".to_string();
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

                    // Partition: *web_app* => is_managed=false, *internally* => is_managed=true
                    let (web_app, internal): (Vec<_>, Vec<_>) =
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

                    // Build final output
                    let mut output = String::new();
                    output.push_str("Current Redeems (twitch-eventsub):\n\n");

                    // “web-app managed”
                    if web_app.is_empty() {
                        output.push_str("[Web-app managed redeems]\n(no items)\n\n");
                    } else {
                        output.push_str("[Web-app managed redeems]\n");
                        output.push_str(&format_table(&web_app));
                        output.push_str("\n");
                    }

                    // “internally managed”
                    if internal.is_empty() {
                        output.push_str("[Internally managed redeems]\n(no items)\n\n");
                    } else {
                        output.push_str("[Internally managed redeems]\n");
                        output.push_str(&format_table(&internal));
                        output.push_str("\n");
                    }
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
                return "Usage: redeem info <redeemNameOrUuid>".to_string();
            }
            let user_input = args[1];
            match resolve_redeems_by_arg(bot_api, user_input).await {
                Ok(matches) => {
                    if matches.len() == 1 {
                        // Exactly one match => show details
                        let rd = &matches[0];
                        format_redeem_details(rd)
                    } else if matches.is_empty() {
                        format!("No redeem found matching '{user_input}'")
                    } else {
                        // Multiple matches => show them all
                        let mut msg = String::new();
                        msg.push_str("Multiple redeems match that identifier:\n\n");
                        msg.push_str(&format_table(&matches));
                        msg.push_str("\nPlease specify the exact UUID to see details of one.\n");
                        msg
                    }
                }
                Err(e) => format!("Error: {e}"),
            }
        }

        // -----------------------------------------------------
        // ADD
        // -----------------------------------------------------
        "add" => {
            // Example usage:
            //   redeem add <rewardName> <cost> [--managed] [--offline] [--dynamic] [--input]
            if args.len() < 3 {
                return "Usage: redeem add <rewardName> <cost> [--managed] [--offline] [--dynamic] [--input]".to_string();
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
            let mut is_user_input_required = false;
            for flag in &args[3..] {
                match flag.to_lowercase().as_str() {
                    "--managed" => is_managed = true,
                    "--offline" => active_offline = true,
                    "--dynamic" => dynamic_pricing = true,
                    "--input" => is_user_input_required = true,
                    other => {
                        return format!("Unknown flag '{other}'. Valid flags: --managed, --offline, --dynamic, --input");
                    }
                }
            }

            // 1) Insert new row. We'll pass empty "" for reward_id initially.
            match bot_api.create_redeem("twitch-eventsub", "", reward_name, cost, dynamic_pricing).await {
                Ok(mut new_rd) => {
                    // 2) If user wants is_managed, offline, or user_input_required, update the newly created row
                    if is_managed || active_offline || is_user_input_required {
                        new_rd.is_managed = is_managed;
                        new_rd.active_offline = active_offline;
                        new_rd.is_user_input_required = is_user_input_required;
                        new_rd.updated_at = Utc::now();

                        if let Err(e) = bot_api.update_redeem(&new_rd).await {
                            return format!("(Partial) Created redeem but error updating additional flags => {e}");
                        }
                    }

                    format!(
                        "New redeem '{}' created with cost={} (managed={}, offline={}, dynamic={}, input={}).\n\
                         You can run 'redeem sync' or restart to push it to Twitch if managed=true.",
                        new_rd.reward_name, new_rd.cost, is_managed, active_offline, dynamic_pricing, is_user_input_required
                    )
                }
                Err(e) => format!("Error creating => {e}"),
            }
        }

        // -----------------------------------------------------
        // ENABLE (set is_active=true)
        // -----------------------------------------------------
        "enable" => {
            if args.len() < 2 {
                return "Usage: redeem enable <redeemNameOrUuid>".to_string();
            }
            let user_input = args[1];
            match resolve_singleton_redeem(bot_api, user_input).await {
                Ok(r) => {
                    if let Err(e) = bot_api.set_redeem_active(r.redeem_id, true).await {
                        return format!("Error enabling => {e}");
                    }
                    format!("Redeem '{}' is now enabled.", r.reward_name)
                }
                Err(e) => e,
            }
        }

        // -----------------------------------------------------
        // PAUSE (set is_active=false)
        // -----------------------------------------------------
        "pause" => {
            if args.len() < 2 {
                return "Usage: redeem pause <redeemNameOrUuid>".to_string();
            }
            let user_input = args[1];
            match resolve_singleton_redeem(bot_api, user_input).await {
                Ok(r) => {
                    if let Err(e) = bot_api.set_redeem_active(r.redeem_id, false).await {
                        return format!("Error pausing => {e}");
                    }
                    format!("Redeem '{}' has been paused (is_active=false).", r.reward_name)
                }
                Err(e) => e,
            }
        }

        // -----------------------------------------------------
        // OFFLINE (toggle `active_offline`)
        // -----------------------------------------------------
        "offline" => {
            if args.len() < 2 {
                return "Usage: redeem offline <redeemNameOrUuid>".to_string();
            }
            let user_input = args[1];
            match resolve_singleton_redeem(bot_api, user_input).await {
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
                Err(e) => e,
            }
        }

        // -----------------------------------------------------
        // SETCOST
        // -----------------------------------------------------
        "setcost" => {
            if args.len() < 3 {
                return "Usage: redeem setcost <points> <redeemNameOrUuid>".to_string();
            }
            let cost_str = args[1];
            let user_input = args[2];
            let cost = match cost_str.parse::<i32>() {
                Ok(n) => n,
                Err(_) => return "Cost must be an integer.".to_string(),
            };

            match resolve_singleton_redeem(bot_api, user_input).await {
                Ok(r) => {
                    if let Err(e) = bot_api.update_redeem_cost(r.redeem_id, cost).await {
                        return format!("Error setting cost => {e}");
                    }
                    format!("Redeem '{}' cost set to {}.", r.reward_name, cost)
                }
                Err(e) => e,
            }
        }

        // -----------------------------------------------------
        // SETPROMPT (demo only)
        // -----------------------------------------------------
        "setprompt" => {
            if args.len() < 3 {
                return "Usage: redeem setprompt <promptText> <redeemNameOrUuid>".to_string();
            }
            let prompt_text = args[1];
            let user_input = args[2];
            match resolve_redeems_by_arg(bot_api, user_input).await {
                Ok(matches) => {
                    if matches.len() == 1 {
                        let r = &matches[0];
                        format!("(Demo) Would set prompt for '{}' => '{}'", r.reward_name, prompt_text)
                    } else if matches.is_empty() {
                        format!("No redeem found matching '{user_input}'")
                    } else {
                        let mut msg = String::new();
                        msg.push_str("Multiple redeems match that identifier:\n\n");
                        msg.push_str(&format_table(&matches));
                        msg.push_str("\nPlease specify the exact UUID.\n");
                        msg
                    }
                }
                Err(e) => format!("Error: {e}"),
            }
        }

        // -----------------------------------------------------
        // SETPLUGIN
        // -----------------------------------------------------
        "setplugin" => {
            if args.len() < 3 {
                return "Usage: redeem setplugin <pluginName> <redeemNameOrUuid>".to_string();
            }
            let plugin_name = args[1];
            let user_input = args[2];
            match resolve_singleton_redeem(bot_api, user_input).await {
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
                Err(e) => e,
            }
        }

        // -----------------------------------------------------
        // SETCOMMAND
        // -----------------------------------------------------
        "setcommand" => {
            if args.len() < 3 {
                return "Usage: redeem setcommand <commandName> <redeemNameOrUuid>".to_string();
            }
            let command_name = args[1];
            let user_input = args[2];
            match resolve_singleton_redeem(bot_api, user_input).await {
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
                Err(e) => e,
            }
        }

        // -----------------------------------------------------
        // SETINPUT (toggle is_user_input_required)
        // -----------------------------------------------------
        "setinput" => {
            if args.len() < 2 {
                return "Usage: redeem setinput <redeemNameOrUuid>".to_string();
            }
            let user_input = args[1];
            match resolve_singleton_redeem(bot_api, user_input).await {
                Ok(mut redeem) => {
                    let new_val = !redeem.is_user_input_required;
                    redeem.is_user_input_required = new_val;
                    redeem.updated_at = Utc::now();
                    let update_result = bot_api.update_redeem(&redeem).await;
                    match update_result {
                        Ok(_) => format!(
                            "Redeem '{}' user input required toggled to {}.",
                            redeem.reward_name, redeem.is_user_input_required
                        ),
                        Err(e) => format!("Error updating => {e}"),
                    }
                }
                Err(e) => e,
            }
        }

        // REMOVE
        // -----------------------------------------------------
        "remove" => {
            if args.len() < 2 {
                return "Usage: redeem remove <redeemNameOrUuid>".to_string();
            }
            let user_input = args[1];
            match resolve_singleton_redeem(bot_api, user_input).await {
                Ok(r) => {
                    if let Err(e) = bot_api.delete_redeem(r.redeem_id).await {
                        return format!("Error removing => {e}");
                    }
                    format!("Redeem '{}' removed from DB.", r.reward_name)
                }
                Err(e) => e,
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

/// Attempts to resolve exactly one redeem for commands that require a single target.
///
/// - If `user_input` is parseable as a UUID, we look up that one redeem.
/// - Else we search by name (case-insensitive).
/// - If multiple matches by name, we return an error string listing them.
/// - If exactly 1 match, we return it.
/// - If none, we return an error string.
async fn resolve_singleton_redeem(bot_api: &Arc<dyn BotApi>, user_input: &str) -> Result<Redeem, String> {
    let matches = resolve_redeems_by_arg(bot_api, user_input).await
        .map_err(|e| format!("{e}"))?;

    match matches.len() {
        0 => Err(format!("No redeem found matching '{user_input}'.")),
        1 => Ok(matches[0].clone()),
        _ => {
            // multiple
            let mut msg = String::from("Multiple redeems match that identifier:\n\n");
            msg.push_str(&format_table(&matches));
            msg.push_str("\nPlease specify the exact UUID.\n");
            Err(msg)
        }
    }
}

/// Finds all redeems that match the given string either as a UUID or by reward_name.
/// Returns a vector of matches (could be empty, 1, or multiple).
async fn resolve_redeems_by_arg(
    bot_api: &Arc<dyn BotApi>,
    user_input: &str
) -> Result<Vec<Redeem>, String> {
    // First, attempt to parse as a UUID
    if let Ok(u) = Uuid::parse_str(user_input) {
        // If that parse works, fetch all redeems once...
        let all = bot_api.list_redeems("twitch-eventsub").await
            .map_err(|e| format!("Error listing redeems => {e}"))?;

        // Then find the one with that ID (if any).
        let filtered: Vec<Redeem> = all.into_iter()
            .filter(|r| r.redeem_id == u)
            .collect();

        return Ok(filtered);
    }

    // Otherwise, treat as a name. We do a case-insensitive compare:
    let lowered = user_input.to_lowercase();
    let all = bot_api.list_redeems("twitch-eventsub").await
        .map_err(|e| format!("Error listing redeems => {e}"))?;

    let filtered: Vec<Redeem> = all.into_iter()
        .filter(|r| r.reward_name.to_lowercase() == lowered)
        .collect();

    Ok(filtered)
}

/// Helper to pretty-print a table of Redeems (for `redeem list`).
///
/// Now includes the UUID at the end of each line, with updated headers:
fn format_table(redeems: &[Redeem]) -> String {
    if redeems.is_empty() {
        return "(none)\n".to_string();
    }

    // 1) Collect data: [Name, Cost, Actv, Offl, Plugin, Command, UUID]
    let mut rows: Vec<[String; 7]> = Vec::new();
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
            rd.redeem_id.to_string(), // Include UUID at the end
        ]);
    }

    // 2) Determine column widths
    let mut col_widths = [0usize; 7];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(cell.len());
        }
    }
    // Add a bit of padding for readability
    for w in &mut col_widths {
        *w += 2;
    }

    let headers = ["Name", "Cost", "Actv", "Offl", "Plugin", "Command", "UUID"];
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
                // Right-align cost
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
         redeem_id:             {}\n\
         platform:              {}\n\
         reward_id:             {}\n\
         reward_name:           {}\n\
         cost:                  {}\n\
         is_active:             {}\n\
         dynamic_pricing:       {}\n\
         created_at:            {}\n\
         updated_at:            {}\n\
         active_offline:        {}\n\
         is_managed:            {}\n\
         is_user_input_required: {}\n\
         plugin_name:           {}\n\
         command_name:          {}\n",
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
        rd.is_user_input_required,
        rd.plugin_name.as_deref().unwrap_or("-"),
        rd.command_name.as_deref().unwrap_or("-"),
    )
}
