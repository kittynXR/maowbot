// File: maowbot-tui/src/commands/connectivity.rs

use std::sync::Arc;
use maowbot_core::plugins::bot_api::BotApi;
use tokio::runtime::Handle;

/// “chat on/off” global flags
static mut CHAT_ENABLED: bool = false;
static mut CHAT_PLATFORM_FILTER: Option<String> = None;
static mut CHAT_ACCOUNT_FILTER: Option<String> = None;

/// The main function that handles "autostart", "start", "stop", "chat" commands.
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

    // Example approach: load existing "autostart" from DB
    let cur_val = Handle::current().block_on(async {
        bot_api.get_bot_config_value("autostart").await
    });
    let mut config_json: String = match cur_val {
        Ok(Some(s)) => s,
        _ => String::new()
    };

    // If you have a struct `AutostartConfig`, parse it:
    let mut config_obj: AutostartConfig = if config_json.is_empty() {
        AutostartConfig::new()
    } else {
        match serde_json::from_str(&config_json) {
            Ok(cfg) => cfg,
            Err(_) => AutostartConfig::new(),
        }
    };

    // Now manipulate config_obj
    config_obj.set_platform_account(platform, account, on);

    // Save back
    let new_str = match serde_json::to_string_pretty(&config_obj) {
        Ok(s) => s,
        Err(e) => return format!("Error serializing config => {:?}", e),
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
        return "Usage: start <platform> <accountName>".to_string();
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
        return "Usage: stop <platform> <accountName>".to_string();
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

/// “chat on/off”
fn handle_chat_cmd(args: &[&str]) -> String {
    if args.is_empty() {
        return "Usage: chat <on/off> [platform] [account]".to_string();
    }
    let on_off = args[0];
    let on = on_off.eq_ignore_ascii_case("on");
    let mut platform = None;
    let mut account = None;

    if on && args.len() >= 2 {
        platform = Some(args[1].to_string());
        if args.len() >= 3 {
            account = Some(args[2].to_string());
            set_chat_state(true, platform.clone(), account.clone());
            return format!("Chat ON for platform='{}', account='{}'", args[1], args[2]);
        }
        set_chat_state(true, platform.clone(), None);
        return format!("Chat ON for platform='{}'", args[1]);
    }

    if !on {
        set_chat_state(false, None, None);
        return "Chat OFF".to_string();
    }

    // If "chat on" with no args:
    set_chat_state(true, None, None);
    "Chat ON (no filter)".to_string()
}

/// Example “autostart” config
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AutostartConfig {
    // For example, store platform+account combos
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

/// Inline the set_chat_state function:
fn set_chat_state(on: bool, platform: Option<String>, account: Option<String>) {
    unsafe {
        CHAT_ENABLED = on;
        CHAT_PLATFORM_FILTER = platform;
        CHAT_ACCOUNT_FILTER = account;
    }
}
