// Drip command adapter for TUI
use maowbot_common_ui::{GrpcClient, commands::drip::DripCommands};
use std::sync::Arc;
use crate::tui_module_simple::SimpleTuiModule;

pub async fn handle_drip_command(
    args: &[&str],
    client: &GrpcClient,
    _tui: &Arc<SimpleTuiModule>,
) -> String {
    if args.is_empty() {
        return help_text();
    }

    match args[0] {
        "set" => {
            if args.len() == 1 {
                // show settable
                match DripCommands::show_settable(client).await {
                    Ok(settings) => {
                        let mut output = "=== Drip Settings ===\n".to_string();
                        for (key, value) in settings {
                            output.push_str(&format!("  {} = {}\n", key, value));
                        }
                        output
                    }
                    Err(e) => format!("Error: {}", e),
                }
            } else {
                // e.g. set i/ignore <prefix> or set s/strip <prefix> or set name <value>
                match args[1] {
                    "i" | "ignore" if args.len() > 2 => {
                        match DripCommands::set_ignore_prefix(client, args[2]).await {
                            Ok(result) => format!("Set {} = '{}'", result.setting_type, result.value),
                            Err(e) => format!("Error: {}", e),
                        }
                    }
                    "s" | "strip" if args.len() > 2 => {
                        match DripCommands::set_strip_prefix(client, args[2]).await {
                            Ok(result) => format!("Set {} = '{}'", result.setting_type, result.value),
                            Err(e) => format!("Error: {}", e),
                        }
                    }
                    "name" if args.len() > 2 => {
                        match DripCommands::set_avatar_name(client, args[2]).await {
                            Ok(result) => format!("Set {} = '{}'", result.setting_type, result.value),
                            Err(e) => format!("Error: {}", e),
                        }
                    }
                    _ => "Usage: drip set i/ignore <prefix> | s/strip <prefix> | name <name>".to_string()
                }
            }
        }
        "list" => {
            // drip list => show stored avatars
            // For now, we'll use a default account name. In a real implementation,
            // this would need to determine the current VRChat account
            let account_name = "vrchat_default"; // TODO: Get actual VRChat account name
            
            match DripCommands::list_avatars(client, account_name).await {
                Ok(avs) => {
                    if avs.is_empty() {
                        "No avatars found in drip DB.".to_string()
                    } else {
                        let mut lines = vec!["=== Drip Avatars ===".to_string()];
                        for av in avs {
                            let nm = av.local_name.unwrap_or_else(|| "(none)".to_string());
                            lines.push(format!(" - ID={}  vrcName={} localName={}",
                                               av.vrchat_avatar_id, av.vrchat_avatar_name, nm));
                        }
                        lines.join("\n")
                    }
                }
                Err(e) => format!("Error => {}", e),
            }
        }
        "fit" => {
            if args.len() < 2 {
                return "Usage: drip fit new|add|del|wear ...".to_string();
            }
            match args[1] {
                "new" if args.len() > 2 => {
                    let fit_name = args[2];
                    match DripCommands::fit_new(client, fit_name).await {
                        Ok(_) => format!("Created new fit '{}'", fit_name),
                        Err(e) => format!("Error => {}", e),
                    }
                }
                "add" if args.len() > 4 => {
                    let fit_name = args[2];
                    let param = args[3];
                    let value = args[4];
                    match DripCommands::fit_add_param(client, fit_name, param, value).await {
                        Ok(_) => format!("Added {}={} to fit '{}'", param, value, fit_name),
                        Err(e) => format!("Error => {}", e),
                    }
                }
                "del" if args.len() > 4 => {
                    let fit_name = args[2];
                    let param = args[3];
                    let value = args[4];
                    match DripCommands::fit_del_param(client, fit_name, param, value).await {
                        Ok(_) => format!("Removed {}={} from fit '{}'", param, value, fit_name),
                        Err(e) => format!("Error => {}", e),
                    }
                }
                "w" | "wear" if args.len() > 2 => {
                    let fit_name = args[2];
                    match DripCommands::fit_wear(client, fit_name).await {
                        Ok(fit) => {
                            let mut output = format!("Wearing fit '{}'\n", fit.name);
                            if !fit.parameters.is_empty() {
                                output.push_str("Parameters:\n");
                                for (param, value) in fit.parameters {
                                    output.push_str(&format!("  {} = {}\n", param, value));
                                }
                            }
                            output
                        }
                        Err(e) => format!("Error => {}", e),
                    }
                }
                _ => "Usage: drip fit new <name> | add <name> <param> <value> | del <name> <param> <value> | wear <name>".to_string()
            }
        }
        "props" => {
            if args.len() < 2 {
                return "Usage: drip props add|del|timer ...".to_string();
            }
            match args[1] {
                "add" if args.len() > 4 => {
                    let prop_name = args[2];
                    let param = args[3];
                    let value = args[4];
                    match DripCommands::props_add(client, prop_name, param, value).await {
                        Ok(_) => format!("Added {}={} to prop '{}'", param, value, prop_name),
                        Err(e) => format!("Error => {}", e),
                    }
                }
                "del" if args.len() > 4 => {
                    let prop_name = args[2];
                    let param = args[3];
                    let value = args[4];
                    match DripCommands::props_del(client, prop_name, param, value).await {
                        Ok(_) => format!("Removed {}={} from prop '{}'", param, value, prop_name),
                        Err(e) => format!("Error => {}", e),
                    }
                }
                "timer" if args.len() > 3 => {
                    let prop_name = args[2];
                    let timer_data = args[3];
                    match DripCommands::props_timer(client, prop_name, timer_data).await {
                        Ok(_) => format!("Set timer '{}' for prop '{}'", timer_data, prop_name),
                        Err(e) => format!("Error => {}", e),
                    }
                }
                _ => "Usage: drip props add <name> <param> <value> | del <name> <param> <value> | timer <name> <timeData>".to_string()
            }
        }
        _ => help_text(),
    }
}

fn help_text() -> String {
    r#"Usage: drip <subcommand> ...
  set i/ignore <prefix>
  set s/strip <prefix>
  set name <newName>
  list
  fit new <name>
  fit add <name> <paramName> <paramValue>
  fit del <name> <paramName> <paramValue>
  fit wear <name>
  props add <propName> <paramName> <paramValue>
  props del <propName> <paramName> <paramValue>
  props timer <propName> <timeData>
"#.to_string()
}