// maowbot-tui/src/commands/connectivity.rs
use std::sync::Arc;
use crate::tui_module::tui_block_on;
use maowbot_core::plugins::bot_api::BotApi;
use serde_json::{Map, Value};
use maowbot_core::tasks::autostart::{AutostartConfig};

/// We define a simple "chat streaming" global state in TUI to decide if we show inbound messages.
static mut CHAT_ENABLED: bool = false;
static mut CHAT_PLATFORM_FILTER: Option<String> = None;
static mut CHAT_ACCOUNT_FILTER: Option<String> = None;

/// Handle connectivity commands:
///   autostart <on/off> <platform> <accountName>
///   start <platform> <accountName>
///   stop <platform> <accountName>
///   chat <on/off> [platform] [accountName]
///
/// (Within the TUI, we'll keep it simple.)
pub fn handle_connectivity_command(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.is_empty() {
        return "Usage:\n  autostart <on/off> <platform> <account>\n  start <platform> <account>\n  stop <platform> <account>\n  chat <on/off> [platform] [account]\n".to_string();
    }
    match args[0] {
        "autostart" => handle_autostart_cmd(&args[1..], bot_api),
        "start" => handle_start_cmd(&args[1..], bot_api),
        "stop"  => handle_stop_cmd(&args[1..], bot_api),
        "chat"  => handle_chat_cmd(&args[1..]),
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

    // We'll fetch the current "autostart" value from DB
    let cur_val = tui_block_on(bot_api.get_bot_config_value("autostart"));
    let mut config = if let Ok(Some(json_str)) = cur_val {
        serde_json::from_str::<AutostartConfig>(&json_str).unwrap_or_else(|_| AutostartConfig::new())
    } else {
        AutostartConfig::new()
    };

    config.set_platform_account(platform, account, on);

    // save back
    let new_str = serde_json::to_string_pretty(&config).unwrap_or_default();
    if let Err(e) = tui_block_on(bot_api.set_bot_config_value("autostart", &new_str)) {
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
        return "Usage: start <platform> <accountName>".to_string();
    }
    let platform = args[0];
    let account = args[1];
    let res = tui_block_on(bot_api.start_platform_runtime(platform, account));
    match res {
        Ok(_) => format!("Started platform='{}', account='{}'", platform, account),
        Err(e) => format!("Error => {:?}", e),
    }
}

fn handle_stop_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: stop <platform> <accountName>".to_string();
    }
    let platform = args[0];
    let account = args[1];
    let res = tui_block_on(bot_api.stop_platform_runtime(platform, account));
    match res {
        Ok(_) => format!("Stopped platform='{}', account='{}'", platform, account),
        Err(e) => format!("Error => {:?}", e),
    }
}

fn handle_chat_cmd(args: &[&str]) -> String {
    if args.is_empty() {
        return "Usage: chat <on/off> [platform] [account]".to_string();
    }
    let on_off = args[0];
    let on = on_off.eq_ignore_ascii_case("on");
    unsafe {
        CHAT_ENABLED = on;
        CHAT_PLATFORM_FILTER = None;
        CHAT_ACCOUNT_FILTER = None;
    }
    if on && args.len() >= 2 {
        let platform = args[1];
        unsafe {
            CHAT_PLATFORM_FILTER = Some(platform.to_string());
        }
        if args.len() >= 3 {
            let account = args[2];
            unsafe {
                CHAT_ACCOUNT_FILTER = Some(account.to_string());
            }
            return format!("Chat ON for platform='{}', account='{}'", platform, account);
        }
        return format!("Chat ON for platform='{}'", platform);
    }
    if !on {
        return "Chat OFF".to_string();
    }
    // If they typed "chat on" with no platform, we do not filter anything.
    "Chat ON (no filter)".to_string()
}
