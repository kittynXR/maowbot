use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;
use tokio::runtime::Handle;

use crate::tui_module::TuiModule;

/// The main function that handles "autostart", "start", "stop", "chat" commands.
pub fn handle_connectivity_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return "Usage:\n  autostart <on/off> <platform> <account>\n  start <platform> <account>\n  stop <platform> <account>\n  chat <on/off> [platform] [account]\n".to_string();
    }
    match args[0] {
        "autostart" => handle_autostart_cmd(&args[1..], bot_api),
        "start"     => handle_start_cmd(&args[1..], bot_api),
        "stop"      => handle_stop_cmd(&args[1..], bot_api),
        "chat"      => handle_chat_cmd(&args[1..], tui_module),
        _ => "Unknown connectivity command. See usage:\n  autostart\n  start\n  stop\n  chat\n".to_string(),
    }
}

fn handle_autostart_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 3 {
        return "Usage: autostart <on/off> <platform> <account>".to_string();
    }
    let on_off = args[0];
    let platform = args[1];
    let account = args[2];

    let on = match on_off.to_lowercase().as_str() {
        "on" => true,
        "off" => false,
        _ => return "Usage: autostart <on/off> <platform> <account>".to_string(),
    };

    let cur_val = Handle::current().block_on(async {
        bot_api.get_bot_config_value("autostart").await
    });

    let mut config_json = match cur_val {
        Ok(Some(s)) => s,
        _ => String::new(),
    };

    let mut config_obj: AutostartConfig = if config_json.is_empty() {
        AutostartConfig::new()
    } else {
        match serde_json::from_str(&config_json) {
            Ok(cfg) => cfg,
            Err(_) => AutostartConfig::new(),
        }
    };

    config_obj.set_platform_account(platform, account, on);

    let new_str = match serde_json::to_string_pretty(&config_obj) {
        Ok(s) => s,
        Err(e) => return format!("Error serializing autostart => {:?}", e),
    };

    let set_res = Handle::current().block_on(async {
        bot_api.set_bot_config_value("autostart", &new_str).await
    });
    if let Err(e) = set_res {
        return format!("Error saving autostart => {:?}", e);
    }

    if on {
        format!("Autostart enabled for platform='{}', account='{}'", platform, account)
    } else {
        format!("Autostart disabled for platform='{}', account='{}'", platform, account)
    }
}

fn handle_start_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: start <platform> <account>".to_string();
    }
    let platform = args[0];
    let account = args[1];

    let res = Handle::current().block_on(async {
        bot_api.start_platform_runtime(platform, account).await
    });
    match res {
        Ok(_) => format!("Started platform='{}', account='{}'", platform, account),
        Err(e) => format!("Error => {:?}", e),
    }
}

fn handle_stop_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: stop <platform> <account>".to_string();
    }
    let platform = args[0];
    let account = args[1];

    let res = Handle::current().block_on(async {
        bot_api.stop_platform_runtime(platform, account).await
    });
    match res {
        Ok(_) => format!("Stopped platform='{}', account='{}'", platform, account),
        Err(e) => format!("Error => {:?}", e),
    }
}

/// Adjusted logic so:
///   chat <on/off>
///     => If *no further arguments*, turns chat feed on/off for *all* platforms+accounts.
///   chat <on/off> <platform>
///     => Limit feed to that one platform (any account).
///   chat <on/off> <platform> <account>
///     => Limit feed to that one platform+account.
fn handle_chat_cmd(args: &[&str], tui_module: &Arc<TuiModule>) -> String {
    if args.is_empty() {
        return "Usage: chat <on/off> [platform] [account]".to_string();
    }

    let on_off = args[0].to_lowercase();
    let on = on_off == "on";

    // Because `args.len()` can be 1, 2, or 3, we interpret accordingly:
    let (platform_filter, account_filter) = match args.len() {
        1 => {
            // e.g. user typed "chat on" => no filters => all platforms
            (None, None)
        }
        2 => {
            // e.g. user typed "chat on twitch" => filter only that platform
            (Some(args[1].to_string()), None)
        }
        _ => {
            // e.g. "chat on twitch myChannel"
            (Some(args[1].to_string()), Some(args[2].to_string()))
        }
    };

    // Update the TUI’s chat_state asynchronously
    Handle::current().block_on(async {
        tui_module.set_chat_state(on, platform_filter.clone(), account_filter.clone()).await;
    });

    if on {
        match (platform_filter, account_filter) {
            (None, None) => "Chat ON for ALL platforms/accounts".to_string(),
            (Some(p), None) => format!("Chat ON for platform='{}' (any account)", p),
            (Some(p), Some(a)) => format!("Chat ON for platform='{}', account='{}'", p, a),
            _ => unreachable!(),
        }
    } else {
        "Chat OFF".to_string()
    }
}

/// Example “autostart” config object (abbreviated here).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AutostartConfig {
    pub accounts: Vec<(String, String)>,
}
impl AutostartConfig {
    pub fn new() -> Self {
        Self { accounts: vec![] }
    }
    pub fn set_platform_account(&mut self, platform: &str, acct: &str, on: bool) {
        if on {
            if !self.accounts.iter().any(|(p, a)| p == platform && a == acct) {
                self.accounts.push((platform.to_string(), acct.to_string()));
            }
        } else {
            self.accounts.retain(|(p, a)| !(p == platform && a == acct));
        }
    }
}