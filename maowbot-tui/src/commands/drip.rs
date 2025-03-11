// maowbot-tui/src/commands/drip.rs
//
// Handler for the "drip" command in the TUI.
// Delegates to DripApi methods on BotApi.

use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;
use maowbot_core::plugins::bot_api::drip_api::DripApi;
use crate::tui_module::TuiModule;

pub async fn handle_drip_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    _tui: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return help_text();
    }

    // Because drip is part of the DripApi subtrait, we downcast to DripApi:
    let drip_api = bot_api.clone();

    match args[0] {
        "set" => {
            if args.len() == 1 {
                // show settable
                match drip_api.drip_show_settable().await {
                    Ok(msg) => msg,
                    Err(e) => format!("Error: {:?}", e),
                }
            } else {
                // e.g. set i/ignore <prefix> or set s/strip <prefix> or set name <value>
                match args[1] {
                    "i" | "ignore" if args.len() > 2 => {
                        match drip_api.drip_set_ignore_prefix(args[2]).await {
                            Ok(msg) => msg,
                            Err(e) => format!("Error: {:?}", e),
                        }
                    }
                    "s" | "strip" if args.len() > 2 => {
                        match drip_api.drip_set_strip_prefix(args[2]).await {
                            Ok(msg) => msg,
                            Err(e) => format!("Error: {:?}", e),
                        }
                    }
                    "name" if args.len() > 2 => {
                        match drip_api.drip_set_avatar_name(args[2]).await {
                            Ok(msg) => msg,
                            Err(e) => format!("Error: {:?}", e),
                        }
                    }
                    _ => "Usage: drip set i/ignore <prefix> | s/strip <prefix> | name <name>".to_string()
                }
            }
        }
        "list" => {
            // drip list => show stored avatars
            match drip_api.drip_list_avatars().await {
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
                Err(e) => format!("Error => {:?}", e),
            }
        }
        "fit" => {
            if args.len() < 2 {
                return "Usage: drip fit new|add|del|wear ...".to_string();
            }
            match args[1] {
                "new" if args.len() > 2 => {
                    let fit_name = args[2];
                    match drip_api.drip_fit_new(fit_name).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error => {:?}", e),
                    }
                }
                "add" if args.len() > 4 => {
                    let fit_name = args[2];
                    let param = args[3];
                    let value = args[4];
                    match drip_api.drip_fit_add_param(fit_name, param, value).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error => {:?}", e),
                    }
                }
                "del" if args.len() > 4 => {
                    let fit_name = args[2];
                    let param = args[3];
                    let value = args[4];
                    match drip_api.drip_fit_del_param(fit_name, param, value).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error => {:?}", e),
                    }
                }
                "w" | "wear" if args.len() > 2 => {
                    let fit_name = args[2];
                    match drip_api.drip_fit_wear(fit_name).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error => {:?}", e),
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
                    match drip_api.drip_props_add(prop_name, param, value).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error => {:?}", e),
                    }
                }
                "del" if args.len() > 4 => {
                    let prop_name = args[2];
                    let param = args[3];
                    let value = args[4];
                    match drip_api.drip_props_del(prop_name, param, value).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error => {:?}", e),
                    }
                }
                "timer" if args.len() > 3 => {
                    let prop_name = args[2];
                    let timer_data = args[3];
                    match drip_api.drip_props_timer(prop_name, timer_data).await {
                        Ok(msg) => msg,
                        Err(e) => format!("Error => {:?}", e),
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