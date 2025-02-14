// File: maowbot-tui/src/commands/connectivity.rs

use std::sync::Arc;
use maowbot_core::tasks::autostart::AutostartConfig;
use maowbot_core::plugins::bot_api::BotApi;
use crate::tui_module::TuiModule;

/// Handles "autostart", "start", "stop", "chat" commands asynchronously.
pub async fn handle_connectivity_command(
    args: &[&str],
    bot_api: &Arc<dyn BotApi>,
    tui_module: &Arc<TuiModule>,
) -> String {
    if args.is_empty() {
        return "Usage:\n  autostart <on/off> <platform> <account>\n  start <platform> <account>\n  stop <platform> <account>\n  chat <on/off> [platform] [account]\n".to_string();
    }

    match args[0] {
        "autostart" => handle_autostart_cmd(&args[1..], bot_api).await,
        "start"     => handle_start_cmd(&args[1..], bot_api).await,
        "stop"      => handle_stop_cmd(&args[1..], bot_api).await,
        "chat"      => handle_chat_cmd(&args[1..], tui_module).await,
        _ => "Unknown connectivity command. See usage:\n  autostart\n  start\n  stop\n  chat\n".to_string(),
    }
}

async fn handle_autostart_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
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

    let current_val = bot_api.get_bot_config_value("autostart").await;
    let config_json = match current_val {
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

    if let Err(e) = bot_api.set_bot_config_value("autostart", &new_str).await {
        return format!("Error saving autostart => {:?}", e);
    }

    if on {
        format!("Autostart enabled for platform='{}', account='{}'", platform, account)
    } else {
        format!("Autostart disabled for platform='{}', account='{}'", platform, account)
    }
}

async fn handle_start_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: start <platform> <account>".to_string();
    }
    let platform = args[0];
    let account = args[1];

    match bot_api.start_platform_runtime(platform, account).await {
        Ok(_) => format!("Started platform='{}', account='{}'", platform, account),
        Err(e) => format!("Error => {:?}", e),
    }
}

async fn handle_stop_cmd(args: &[&str], bot_api: &Arc<dyn BotApi>) -> String {
    if args.len() < 2 {
        return "Usage: stop <platform> <account>".to_string();
    }
    let platform = args[0];
    let account = args[1];

    match bot_api.stop_platform_runtime(platform, account).await {
        Ok(_) => format!("Stopped platform='{}', account='{}'", platform, account),
        Err(e) => format!("Error => {:?}", e),
    }
}

/// chat <on/off> [platform] [account]
async fn handle_chat_cmd(args: &[&str], tui_module: &Arc<TuiModule>) -> String {
    if args.is_empty() {
        return "Usage: chat <on/off> [platform] [account]".to_string();
    }
    let on_off = args[0].to_lowercase();
    let on = on_off == "on";

    let (pf, af) = match args.len() {
        1 => (None, None),
        2 => (Some(args[1].to_string()), None),
        _ => (Some(args[1].to_string()), Some(args[2].to_string())),
    };

    tui_module.set_chat_state(on, pf.clone(), af.clone()).await;

    if on {
        match (pf, af) {
            (None, None) => "Chat ON for ALL platforms/accounts".to_string(),
            (Some(p), None) => format!("Chat ON for platform='{}' (any account)", p),
            (Some(p), Some(a)) => format!("Chat ON for platform='{}', account='{}'", p, a),
            _ => unreachable!(),
        }
    } else {
        "Chat OFF".to_string()
    }
}