use std::collections::HashMap;
use std::format;
use async_trait::async_trait;
use tokio::sync::mpsc;
use twilight_model::id::marker::ApplicationMarker;
use uuid::Uuid;
use crate::error::Error;
use crate::models::{Command, CommandUsage, Redeem, RedeemUsage, UserAnalysis};
use crate::models::analytics::{BotEvent, ChatMessage};
use crate::models::auth::Platform;
use crate::models::discord::{DiscordAccountRecord, DiscordChannelRecord, DiscordEmbed, DiscordEventConfigRecord, DiscordGuildRecord, DiscordLiveRoleRecord};
use crate::models::drip::DripAvatarSummary;
use crate::models::platform::{PlatformConfigData, PlatformCredential, PlatformIdentity};
use crate::models::plugin::StatusData;
use crate::models::user::User;
pub use crate::models::vrchat::{VRChatAvatarBasic, VRChatInstanceBasic, VRChatWorldBasic};

pub trait BotApi:
PluginApi
+ UserApi
+ CredentialsApi
+ PlatformApi
+ TwitchApi
+ VrchatApi
+ CommandApi
+ RedeemApi
+ OscApi
+ DripApi
+ BotConfigApi
+ DiscordApi
+ AiApi
{
}

impl<T> BotApi for T
where
    T: PluginApi
    + UserApi
    + CredentialsApi
    + PlatformApi
    + TwitchApi
    + VrchatApi
    + CommandApi
    + RedeemApi
    + OscApi
    + DripApi
    + BotConfigApi
    + DiscordApi
    + AiApi,
{
    // marker
}

#[async_trait]
pub trait CommandApi: Send + Sync {
    async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error>;
    async fn create_command(&self, platform: &str, command_name: &str, min_role: &str) -> Result<Command, Error>;
    async fn set_command_active(&self, command_id: Uuid, is_active: bool) -> Result<(), Error>;
    async fn update_command_role(&self, command_id: Uuid, new_role: &str) -> Result<(), Error>;
    async fn delete_command(&self, command_id: Uuid) -> Result<(), Error>;
    async fn get_usage_for_command(&self, command_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
    async fn get_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
    async fn update_command(&self, updated_cmd: &Command) -> Result<(), Error>;
}

#[async_trait]
pub trait CredentialsApi: Send + Sync {
    async fn begin_auth_flow(&self, platform: Platform, is_bot: bool) -> Result<String, Error>;
    async fn complete_auth_flow(
        &self,
        platform: Platform,
        code: String
    ) -> Result<PlatformCredential, Error>;
    async fn complete_auth_flow_for_user(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;
    async fn complete_auth_flow_for_user_multi(
        &self,
        platform: Platform,
        user_id: Uuid,
        keys: HashMap<String, String>,
    ) -> Result<PlatformCredential, Error>;
    async fn complete_auth_flow_for_user_2fa(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;
    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<(), Error>;
    async fn refresh_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<PlatformCredential, Error>;
    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error>;
    async fn store_credential(&self, cred: PlatformCredential) -> Result<(), Error>;
}

#[async_trait]
pub trait DripApi: Send + Sync {
    async fn drip_show_settable(&self) -> Result<String, Error>;
    async fn drip_set_ignore_prefix(&self, prefix: &str) -> Result<String, Error>;
    async fn drip_set_strip_prefix(&self, prefix: &str) -> Result<String, Error>;
    async fn drip_set_avatar_name(&self, new_name: &str) -> Result<String, Error>;
    async fn drip_list_avatars(&self) -> Result<Vec<DripAvatarSummary>, Error>;
    async fn drip_fit_new(&self, fit_name: &str) -> Result<String, Error>;
    async fn drip_fit_add_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;
    async fn drip_fit_del_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;
    async fn drip_fit_wear(&self, fit_name: &str) -> Result<String, Error>;
    async fn drip_props_add(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;
    async fn drip_props_del(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;
    async fn drip_props_timer(&self, prop_name: &str, timer_data: &str) -> Result<String, Error>;
}

#[async_trait]
pub trait OscApi: Send + Sync {
    async fn osc_start(&self) -> Result<(), Error>;
    async fn osc_stop(&self) -> Result<(), Error>;
    async fn osc_restart(&self) -> Result<(), Error> {
        self.osc_stop().await?;
        self.osc_start().await
    }
    async fn osc_status(&self) -> Result<crate::models::osc::OscStatus, Error>;
    async fn osc_chatbox(&self, message: &str) -> Result<(), Error>;
    async fn osc_discover_peers(&self) -> Result<Vec<String>, Error>;
    // Need to add to OscApi trait:
    // Add to the OscApi trait
    async fn osc_take_raw_receiver(&self) -> Result<Option<mpsc::UnboundedReceiver<rosc::OscPacket>>, Error>;
    
    // OSC parameter sending methods
    async fn osc_send_avatar_parameter_bool(&self, name: &str, value: bool) -> Result<(), Error>;
    async fn osc_send_avatar_parameter_int(&self, name: &str, value: i32) -> Result<(), Error>;
    async fn osc_send_avatar_parameter_float(&self, name: &str, value: f32) -> Result<(), Error>;
    
    // OSC trigger management methods
    async fn osc_list_triggers(&self) -> Result<Vec<crate::models::osc_toggle::OscTrigger>, Error>;
    async fn osc_list_triggers_with_redeems(&self) -> Result<Vec<(crate::models::osc_toggle::OscTrigger, String)>, Error>;
    async fn osc_get_trigger(&self, trigger_id: i32) -> Result<Option<crate::models::osc_toggle::OscTrigger>, Error>;
    async fn osc_create_trigger(&self, trigger: crate::models::osc_toggle::OscTrigger) -> Result<crate::models::osc_toggle::OscTrigger, Error>;
    async fn osc_update_trigger(&self, trigger: crate::models::osc_toggle::OscTrigger) -> Result<crate::models::osc_toggle::OscTrigger, Error>;
    async fn osc_delete_trigger(&self, trigger_id: i32) -> Result<(), Error>;
    async fn osc_list_active_toggles(&self, user_id: Option<uuid::Uuid>) -> Result<Vec<crate::models::osc_toggle::OscToggleState>, Error>;
    async fn osc_activate_toggle(&self, redeem_id: uuid::Uuid, user_id: uuid::Uuid) -> Result<(), Error>;
}

#[async_trait]
pub trait PlatformApi: Send + Sync {
    async fn create_platform_config(
        &self,
        platform: Platform,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), Error>;

    async fn count_platform_configs_for_platform(
        &self,
        platform_str: String
    ) -> Result<usize, Error>;

    async fn list_platform_configs(
        &self,
        maybe_platform: Option<&str>
    ) -> Result<Vec<PlatformConfigData>, Error>;

    async fn remove_platform_config(
        &self,
        platform_config_id: &str
    ) -> Result<(), Error>;

    async fn start_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;
    async fn stop_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;
}

#[async_trait]
pub trait PluginApi: Send + Sync {
    async fn list_plugins(&self) -> Vec<String>;
    async fn status(&self) -> StatusData;
    async fn shutdown(&self);
    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error>;
    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error>;
    async fn subscribe_chat_events(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent>;
    async fn list_config(&self) -> Result<Vec<(String, String)>, Error>;
}

#[async_trait]
pub trait RedeemApi: Send + Sync {
    async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error>;
    async fn create_redeem(&self, platform: &str, reward_id: &str, reward_name: &str, cost: i32, dynamic: bool)
                           -> Result<Redeem, Error>;
    async fn set_redeem_active(&self, redeem_id: Uuid, is_active: bool) -> Result<(), Error>;
    async fn update_redeem_cost(&self, redeem_id: Uuid, new_cost: i32) -> Result<(), Error>;
    async fn delete_redeem(&self, redeem_id: Uuid) -> Result<(), Error>;
    async fn get_usage_for_redeem(&self, redeem_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn get_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn update_redeem(&self, redeem: &Redeem) -> Result<(), Error>;
    async fn sync_redeems(&self) -> Result<(), Error>;
}

#[async_trait]
pub trait TwitchApi: Send + Sync {
    async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;
    async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;
    async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), Error>;

async fn timeout_twitch_user(&self, account_name: &str, channel: &str, target_user: &str, seconds: u32, reason: Option<&str>, ) -> Result<(), Error>;
}

#[async_trait]
pub trait UserApi: Send + Sync {
    async fn create_user(&self, new_user_id: Uuid, display_name: &str) -> Result<(), Error>;
    async fn remove_user(&self, user_id: Uuid) -> Result<(), Error>;
    async fn get_user(&self, user_id: Uuid) -> Result<Option<User>, Error>;
    async fn update_user_active(&self, user_id: Uuid, is_active: bool) -> Result<(), Error>;
    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error>;
    async fn find_user_by_name(&self, name: &str) -> Result<User, Error>;
    async fn get_user_chat_messages(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        maybe_platform: Option<String>,
        maybe_channel: Option<String>,
        maybe_search: Option<String>,
    ) -> Result<Vec<ChatMessage>, Error>;
    async fn append_moderator_note(&self, user_id: Uuid, note_text: &str) -> Result<(), Error>;
    async fn get_platform_identities_for_user(&self, user_id: Uuid) -> Result<Vec<PlatformIdentity>, Error>;
    async fn get_user_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error>;
    async fn merge_users(
        &self,
        user1_id: Uuid,
        user2_id: Uuid,
        new_global_name: Option<&str>
    ) -> Result<(), Error>;
    async fn add_role_to_user_identity(&self, user_id: Uuid, platform: &str, role: &str) -> Result<(), Error>;
    async fn remove_role_from_user_identity(&self, user_id: Uuid, platform: &str, role: &str) -> Result<(), Error>;
}

#[async_trait]
pub trait VrchatApi: Send + Sync {
    async fn vrchat_get_current_world(&self, account_name: &str) -> Result<VRChatWorldBasic, Error>;
    async fn vrchat_get_current_avatar(&self, account_name: &str) -> Result<VRChatAvatarBasic, Error>;
    async fn vrchat_change_avatar(&self, account_name: &str, new_avatar_id: &str) -> Result<(), Error>;
    async fn vrchat_get_current_instance(&self, account_name: &str) -> Result<VRChatInstanceBasic, Error>;
}

/// ---------------------------------------------------------------------------
/// NEW: BotConfigApi trait
/// ---------------------------------------------------------------------------
#[async_trait]
pub trait BotConfigApi: Send + Sync {
    async fn list_all_config(&self) -> Result<Vec<(String, String)>, Error>;
    async fn get_bot_config_value(&self, config_key: &str) -> Result<Option<String>, Error>;
    async fn set_bot_config_value(&self, config_key: &str, config_value: &str) -> Result<(), Error>;
    async fn delete_bot_config_key(&self, config_key: &str) -> Result<(), Error>;

    async fn set_config_kv_meta(
        &self,
        config_key: &str,
        config_value: &str,
        config_meta: Option<serde_json::Value>,
    ) -> Result<(), Error>;

    async fn get_config_kv_meta(
        &self,
        config_key: &str,
        config_value: &str
    ) -> Result<Option<(String, Option<serde_json::Value>)>, Error>;

    async fn delete_config_kv(&self, config_key: &str, config_value: &str) -> Result<(), Error>;
}

/// ---------------------------------------------------------------------------
/// UPDATED: DiscordApi trait
/// ---------------------------------------------------------------------------
#[async_trait]
pub trait DiscordApi {
    async fn list_discord_guilds(&self, account_name: &str) -> Result<Vec<DiscordGuildRecord>, Error>;
    async fn list_discord_channels(&self, account_name: &str, guild_id: &str) -> Result<Vec<DiscordChannelRecord>, Error>;
    async fn send_discord_message(&self, account_name: &str, server_id: &str, channel_id: &str, text: &str) -> Result<(), Error>;
    async fn send_discord_embed(
        &self,
        account_name: &str,
        server_id: &str,
        channel_id: &str,
        embed: &DiscordEmbed,
        content: Option<&str>
    ) -> Result<(), Error>;
    async fn list_discord_event_configs(&self) -> Result<Vec<DiscordEventConfigRecord>, Error>;
    async fn add_discord_event_config(
        &self,
        event_name: &str,
        guild_id: &str,
        channel_id: &str,
        maybe_credential_id: Option<Uuid>
    ) -> Result<(), Error>;

    async fn remove_discord_event_config(
        &self,
        event_name: &str,
        guild_id: &str,
        channel_id: &str,
        maybe_credential_id: Option<Uuid>
    ) -> Result<(), Error>;

    async fn upsert_discord_account(
        &self,
        account_name: &str,
        credential_id: Option<Uuid>,
        discord_id: Option<&str>
    ) -> Result<(), Error>;

    async fn add_discord_event_role(&self, event_name: &str, role_id: &str) -> Result<(), Error>;
    async fn remove_discord_event_role(&self, event_name: &str, role_id: &str) -> Result<(), Error>;
    async fn list_discord_roles(&self, account_name: &str, guild_id: &str) -> Result<Vec<(String, String)>, Error>;
    
    // Live role management for Twitch streamers
    async fn set_discord_live_role(&self, guild_id: &str, role_id: &str) -> Result<(), Error>;
    async fn get_discord_live_role(&self, guild_id: &str) -> Result<Option<DiscordLiveRoleRecord>, Error>;
    async fn delete_discord_live_role(&self, guild_id: &str) -> Result<(), Error>;
    async fn list_discord_live_roles(&self) -> Result<Vec<DiscordLiveRoleRecord>, Error>;
    
    // Discord role management for users
    async fn add_role_to_discord_user(&self, account_name: &str, guild_id: &str, user_id: &str, role_id: &str) -> Result<(), Error>;
    async fn remove_role_from_discord_user(&self, account_name: &str, guild_id: &str, user_id: &str, role_id: &str) -> Result<(), Error>;
}

/// Trait for AI functionality
#[async_trait]
pub trait AiApi: Send + Sync {
    /// Get direct access to the AI service
    /// This is used for more advanced operations like trigger management
    /// Returns a dyn Any that should be cast to the actual AI service type
    async fn get_ai_service(&self) -> Result<Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>, Error> {
        // Default implementation returns None (no service available)
        Ok(None)
    }
    
    /// Generate a chat completion
    async fn generate_chat(&self, messages: Vec<serde_json::Value>) -> Result<String, Error>;

    async fn generate_with_search(&self, messages: Vec<serde_json::Value>) -> Result<serde_json::Value, Error>;

    /// Generate a completion with function calling
    async fn generate_with_functions(&self, messages: Vec<serde_json::Value>) -> Result<serde_json::Value, Error>;
    
    /// Process a user message with context
    async fn process_user_message(&self, user_id: Uuid, message: &str) -> Result<String, Error>;
    
    /// Register a new function
    async fn register_ai_function(&self, name: &str, description: &str) -> Result<(), Error>;
    
    /// Set the system prompt
    async fn set_system_prompt(&self, prompt: &str) -> Result<(), Error>;
    
    /// Configure an AI provider with the given configuration
    async fn configure_ai_provider(&self, config: serde_json::Value) -> Result<(), Error> {
        // Default implementation just returns an error
        Err(Error::Internal("configure_ai_provider not implemented".to_string()))
    }
}
