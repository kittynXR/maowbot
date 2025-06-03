use maowbot_common::models::{user::User, command::Command, redeem::Redeem};
use chrono::Utc;
use uuid::Uuid;

// User fixtures
pub fn viewer_user() -> User {
    User {
        user_id: Uuid::new_v4(),
        global_username: Some("viewer123".to_string()),
        created_at: Utc::now(),
        last_seen: Utc::now(),
        is_active: true,
    }
}

pub fn moderator_user() -> User {
    User {
        user_id: Uuid::new_v4(),
        global_username: Some("moderator456".to_string()),
        created_at: Utc::now(),
        last_seen: Utc::now(),
        is_active: true,
    }
}

pub fn vip_user() -> User {
    User {
        user_id: Uuid::new_v4(),
        global_username: Some("vip789".to_string()),
        created_at: Utc::now(),
        last_seen: Utc::now(),
        is_active: true,
    }
}

pub fn subscriber_user() -> User {
    User {
        user_id: Uuid::new_v4(),
        global_username: Some("subscriber101".to_string()),
        created_at: Utc::now(),
        last_seen: Utc::now(),
        is_active: true,
    }
}

pub fn broadcaster_user() -> User {
    User {
        user_id: Uuid::new_v4(),
        global_username: Some("broadcaster999".to_string()),
        created_at: Utc::now(),
        last_seen: Utc::now(),
        is_active: true,
    }
}

// Command fixtures
pub fn ping_command() -> Command {
    Command {
        command_id: Uuid::new_v4(),
        platform: "twitch".to_string(),
        command_name: "ping".to_string(),
        min_role: "viewer".to_string(),
        is_active: true,
        cooldown_seconds: 5,
        cooldown_warnonce: false,
        respond_with_credential: None,
        stream_online_only: false,
        stream_offline_only: false,
        active_credential_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn followage_command() -> Command {
    Command {
        command_id: Uuid::new_v4(),
        platform: "twitch".to_string(),
        command_name: "followage".to_string(),
        min_role: "viewer".to_string(),
        is_active: true,
        cooldown_seconds: 10,
        cooldown_warnonce: false,
        respond_with_credential: None,
        stream_online_only: false,
        stream_offline_only: false,
        active_credential_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn vanish_command() -> Command {
    Command {
        command_id: Uuid::new_v4(),
        platform: "twitch".to_string(),
        command_name: "vanish".to_string(),
        min_role: "moderator".to_string(),
        is_active: true,
        cooldown_seconds: 30,
        cooldown_warnonce: false,
        respond_with_credential: None,
        stream_online_only: false,
        stream_offline_only: false,
        active_credential_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn so_command() -> Command {
    Command {
        command_id: Uuid::new_v4(),
        platform: "twitch".to_string(),
        command_name: "so".to_string(),
        min_role: "moderator".to_string(),
        is_active: true,
        cooldown_seconds: 60,
        cooldown_warnonce: false,
        respond_with_credential: None,
        stream_online_only: false,
        stream_offline_only: false,
        active_credential_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// Redeem fixtures
pub fn cute_redeem() -> Redeem {
    Redeem {
        redeem_id: Uuid::new_v4(),
        platform: "twitch".to_string(),
        reward_id: "cute-redeem-123".to_string(),
        reward_name: "Be Cute".to_string(),
        cost: 100,
        is_active: true,
        dynamic_pricing: false,
        active_offline: false,
        is_managed: true,
        plugin_name: Some("builtin".to_string()),
        command_name: Some("cute".to_string()),
        active_credential_id: None,
        is_input_required: false,
        redeem_prompt_text: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn tts_redeem() -> Redeem {
    Redeem {
        redeem_id: Uuid::new_v4(),
        platform: "twitch".to_string(),
        reward_id: "tts-redeem-456".to_string(),
        reward_name: "TTS Message".to_string(),
        cost: 500,
        is_active: true,
        dynamic_pricing: false,
        active_offline: false,
        is_managed: true,
        plugin_name: Some("builtin".to_string()),
        command_name: Some("tts".to_string()),
        active_credential_id: None,
        is_input_required: true,
        redeem_prompt_text: Some("Enter your TTS message".to_string()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn osc_toggle_redeem() -> Redeem {
    Redeem {
        redeem_id: Uuid::new_v4(),
        platform: "twitch".to_string(),
        reward_id: "osc-redeem-789".to_string(),
        reward_name: "Toggle Avatar Feature".to_string(),
        cost: 250,
        is_active: true,
        dynamic_pricing: false,
        active_offline: false,
        is_managed: true,
        plugin_name: Some("builtin".to_string()),
        command_name: Some("osc_toggle".to_string()),
        active_credential_id: None,
        is_input_required: false,
        redeem_prompt_text: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// Test scenario fixtures
pub struct TwitchChatScenario {
    pub messages: Vec<(String, String)>, // (username, message)
}

impl TwitchChatScenario {
    pub fn command_spam() -> Self {
        Self {
            messages: vec![
                ("spammer123".to_string(), "!ping".to_string()),
                ("spammer123".to_string(), "!ping".to_string()),
                ("spammer123".to_string(), "!ping".to_string()),
                ("spammer123".to_string(), "!ping".to_string()),
                ("spammer123".to_string(), "!ping".to_string()),
            ],
        }
    }

    pub fn mixed_chat() -> Self {
        Self {
            messages: vec![
                ("viewer1".to_string(), "Hello chat!".to_string()),
                ("viewer2".to_string(), "!followage".to_string()),
                ("moduser".to_string(), "!so @coolstreamer".to_string()),
                ("subuser".to_string(), "Love the stream!".to_string()),
                ("viewer3".to_string(), "!ping".to_string()),
            ],
        }
    }

    pub fn mod_actions() -> Self {
        Self {
            messages: vec![
                ("moduser".to_string(), "!timeout @baduser 300 Being rude".to_string()),
                ("moduser".to_string(), "!ban @spammer Spamming".to_string()),
                ("moduser".to_string(), "!unban @reformeduser".to_string()),
                ("moduser".to_string(), "!vanish @lurker".to_string()),
            ],
        }
    }
}