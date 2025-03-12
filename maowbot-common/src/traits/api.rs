use std::collections::HashMap;
use async_trait::async_trait;
use tokio::sync::mpsc;
use uuid::Uuid;
use crate::error::Error;
use crate::models::{Command, CommandUsage, Redeem, RedeemUsage, UserAnalysis};
use crate::models::analytics::{BotEvent, ChatMessage};
use crate::models::auth::Platform;
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
{
    // No extra methods; it’s just a “marker” for convenience.
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


/// Credential-related trait: beginning and completing auth flows, storing tokens, etc.
#[async_trait]
pub trait CredentialsApi: Send + Sync {
    /// Step 1 of OAuth or similar flows: returns a URL or instructions to begin.
    async fn begin_auth_flow(&self, platform: Platform, is_bot: bool) -> Result<String, Error>;

    /// Completes the auth flow with a code (no user_id). Typically for “bot” credentials or system credentials.
    async fn complete_auth_flow(
        &self,
        platform: Platform,
        code: String
    ) -> Result<PlatformCredential, Error>;

    /// Completes the auth flow for a specific user (by UUID).
    async fn complete_auth_flow_for_user(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;

    /// Completes the flow with multiple input fields (username/password, or multiple tokens).
    async fn complete_auth_flow_for_user_multi(
        &self,
        platform: Platform,
        user_id: Uuid,
        keys: HashMap<String, String>,
    ) -> Result<PlatformCredential, Error>;

    /// Completes a 2FA step for a user (like VRChat).
    async fn complete_auth_flow_for_user_2fa(
        &self,
        platform: Platform,
        code: String,
        user_id: Uuid
    ) -> Result<PlatformCredential, Error>;

    /// Revoke credentials for a user (by UUID).
    async fn revoke_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<(), Error>;

    /// Refresh credentials for a user (by UUID).
    async fn refresh_credentials(
        &self,
        platform: Platform,
        user_id: String
    ) -> Result<PlatformCredential, Error>;

    /// List all credentials, optionally filtered by platform.
    async fn list_credentials(
        &self,
        maybe_platform: Option<Platform>
    ) -> Result<Vec<PlatformCredential>, Error>;

    /// Store (create/update) an already-built credential in the DB.
    async fn store_credential(&self, cred: PlatformCredential) -> Result<(), Error>;
}


/// Represents a summary or minimal info about an avatar from the drip system.


/// Trait providing "drip" commands.
/// Each method below corresponds to a subcommand or action, e.g.
///   drip set i/ignore <prefix>
#[async_trait]
pub trait DripApi: Send + Sync {
    /// `drip set` => show settable parameters (non-outfit). This might just list rules, etc.
    async fn drip_show_settable(&self) -> Result<String, Error>;

    /// `drip set i/ignore <prefix>`
    async fn drip_set_ignore_prefix(&self, prefix: &str) -> Result<String, Error>;

    /// `drip set s/strip <prefix>`
    async fn drip_set_strip_prefix(&self, prefix: &str) -> Result<String, Error>;

    /// `drip set name <name>` => rename local avatar
    async fn drip_set_avatar_name(&self, new_name: &str) -> Result<String, Error>;

    /// `drip list` => list stored avatars in database
    async fn drip_list_avatars(&self) -> Result<Vec<DripAvatarSummary>, Error>;

    /// `drip fit new <name>` => create new outfit for current avatar
    async fn drip_fit_new(&self, fit_name: &str) -> Result<String, Error>;

    /// `drip fit add <name> <param> <value>`
    async fn drip_fit_add_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip fit del <name> <param> <value>`
    async fn drip_fit_del_param(&self, fit_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip fit w/wear <name>` => sets the parameters for that fit, outputs any missing
    async fn drip_fit_wear(&self, fit_name: &str) -> Result<String, Error>;

    /// `drip props add <prop_name> <param> <value>`
    async fn drip_props_add(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip props del <prop_name> <param> <value>`
    async fn drip_props_del(&self, prop_name: &str, param_name: &str, param_value: &str) -> Result<String, Error>;

    /// `drip props timer <prop_name> <timer_data>`
    async fn drip_props_timer(&self, prop_name: &str, timer_data: &str) -> Result<String, Error>;
}

/// Trait for controlling the OSC manager (start/stop/restart) and sending chatbox messages.
#[async_trait]
pub trait OscApi: Send + Sync {
    /// Starts the OSC system (UDP server, OSCQuery HTTP, etc.).
    async fn osc_start(&self) -> Result<(), Error>;

    /// Stops the OSC system.
    async fn osc_stop(&self) -> Result<(), Error>;

    /// Restarts the OSC system (stop+start).
    async fn osc_restart(&self) -> Result<(), Error> {
        self.osc_stop().await?;
        self.osc_start().await
    }

    /// Returns the current OSC status (running or not, port, etc.).
    async fn osc_status(&self) -> Result<crate::models::osc::OscStatus, Error>;

    /// Sends a single chatbox message to VRChat via OSC (if running).
    async fn osc_chatbox(&self, message: &str) -> Result<(), Error>;

    /// Optionally discover local OSCQuery peers; returns their service names or addresses.
    async fn osc_discover_peers(&self) -> Result<Vec<String>, Error>;
}


/// Sub-trait that deals with platform config (OAuth client_id) and running/stoping connections.
#[async_trait]
pub trait PlatformApi: Send + Sync {
    /// Insert or update a row in “platform_config” for the given platform (client_id, secret).
    async fn create_platform_config(
        &self,
        platform: Platform,
        client_id: String,
        client_secret: Option<String>
    ) -> Result<(), Error>;

    /// Counts how many platform_config rows exist for the given platform string (case-insensitive).
    async fn count_platform_configs_for_platform(
        &self,
        platform_str: String
    ) -> Result<usize, Error>;

    /// Lists all platform_config rows (or just for one platform if `maybe_platform` is provided).
    async fn list_platform_configs(
        &self,
        maybe_platform: Option<&str>
    ) -> Result<Vec<PlatformConfigData>, Error>;

    /// Removes a platform_config row by its UUID (passed as string).
    async fn remove_platform_config(
        &self,
        platform_config_id: &str
    ) -> Result<(), Error>;

    /// Starts the bot’s runtime for a given platform + account.
    async fn start_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;

    /// Stops the bot’s runtime for a given platform + account.
    async fn stop_platform_runtime(&self, platform: &str, account_name: &str) -> Result<(), Error>;

    /// Gets a value from the “bot_config” table (like a key-value store).
    async fn get_bot_config_value(&self, key: &str) -> Result<Option<String>, Error>;

    /// Sets a value in the “bot_config” table.
    async fn set_bot_config_value(&self, key: &str, value: &str) -> Result<(), Error>;
}

#[async_trait]
pub trait PluginApi: Send + Sync {
    /// Returns a list of plugin names. You might label them as “(disabled)” in your logic if wanted.
    async fn list_plugins(&self) -> Vec<String>;

    /// Returns an overall `StatusData` snapshot (which plugins are connected, accounts connected, etc.).
    async fn status(&self) -> StatusData;

    /// Requests that the entire bot shuts down gracefully.
    async fn shutdown(&self);

    /// Toggles a plugin by name: if `enable == true`, enable it; if false, disable it.
    async fn toggle_plugin(&self, plugin_name: &str, enable: bool) -> Result<(), Error>;

    /// Permanently removes a plugin from the system (unloads and deletes from JSON).
    async fn remove_plugin(&self, plugin_name: &str) -> Result<(), Error>;

    /// Subscribe to chat events from the global event bus.
    /// Returns an MPSC receiver that yields `BotEvent::ChatMessage`.
    async fn subscribe_chat_events(&self, buffer_size: Option<usize>) -> mpsc::Receiver<BotEvent>;

    /// Lists all config key/value pairs from some “bot_config” table (if implemented).
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

    // Usage logs
    async fn get_usage_for_redeem(&self, redeem_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn get_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn update_redeem(&self, redeem: &Redeem) -> Result<(), Error>;
    async fn sync_redeems(&self) -> Result<(), Error>;
}

#[async_trait]
pub trait TwitchApi: Send + Sync {
    /// Joins a Twitch IRC channel (e.g. "#somechannel") using the specified account credentials.
    async fn join_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;

    /// Leaves a Twitch IRC channel.
    async fn part_twitch_irc_channel(&self, account_name: &str, channel: &str) -> Result<(), Error>;

    /// Sends a chat message to a Twitch IRC channel.
    async fn send_twitch_irc_message(&self, account_name: &str, channel: &str, text: &str) -> Result<(), Error>;
}


/// Methods for basic user management (create, remove, find, etc.).
#[async_trait]
pub trait UserApi: Send + Sync {
    /// Create a new user row in the DB with the given UUID and display name.
    async fn create_user(&self, new_user_id: Uuid, display_name: &str) -> Result<(), Error>;

    /// Remove a user by UUID.
    async fn remove_user(&self, user_id: Uuid) -> Result<(), Error>;

    /// Fetch a user by UUID, returning None if not found.
    async fn get_user(&self, user_id: Uuid) -> Result<Option<User>, Error>;

    /// Update the user’s `is_active` boolean in the DB.
    async fn update_user_active(&self, user_id: Uuid, is_active: bool) -> Result<(), Error>;

    /// Search users by some textual query. Could be partial string match on username or user_id.
    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error>;

    /// Look up a single user by name, returning an error if not found or if multiple matches.
    async fn find_user_by_name(&self, name: &str) -> Result<User, Error>;

    /// Returns up to `limit` messages from chat_messages for the specified `user_id`,
    /// offset by `offset` for paging, optionally filtered by platform/channel, and text search.
    async fn get_user_chat_messages(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        maybe_platform: Option<String>,
        maybe_channel: Option<String>,
        maybe_search: Option<String>,
    ) -> Result<Vec<ChatMessage>, Error>;

    /// Appends (or sets) a moderator note in user_analysis for the given user_id.
    /// If user_analysis does not exist, create it. Then either append or override the existing note.
    async fn append_moderator_note(&self, user_id: Uuid, note_text: &str) -> Result<(), Error>;

    /// Returns all platform_identities for a given user.
    async fn get_platform_identities_for_user(&self, user_id: Uuid) -> Result<Vec<PlatformIdentity>, Error>;

    /// Returns the user_analysis row if present.
    async fn get_user_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error>;

    // -------------------------------
    // NEW MERGE METHOD
    // -------------------------------
    /// Merge user2’s data into user1, reassigning platform identities and chat messages.
    /// Optionally set a new global_username for user1. Then delete user2 from DB.
    async fn merge_users(
        &self,
        user1_id: Uuid,
        user2_id: Uuid,
        new_global_name: Option<&str>
    ) -> Result<(), Error>;

    async fn add_role_to_user_identity(
        &self,
        user_id: Uuid,
        platform: &str,
        role: &str,
    ) -> Result<(), Error>;

    /// Remove a single role from the user’s platform identity.
    async fn remove_role_from_user_identity(
        &self,
        user_id: Uuid,
        platform: &str,
        role: &str,
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait VrchatApi: Send + Sync {
    /// Returns info about the user’s current VRChat world.
    async fn vrchat_get_current_world(&self, account_name: &str) -> Result<VRChatWorldBasic, Error>;

    /// Returns info about the user’s current VRChat avatar.
    async fn vrchat_get_current_avatar(&self, account_name: &str) -> Result<VRChatAvatarBasic, Error>;

    /// Changes the user’s current avatar to the given `new_avatar_id`.
    async fn vrchat_change_avatar(&self, account_name: &str, new_avatar_id: &str) -> Result<(), Error>;

    /// Returns info about the user’s current instance (world_id + instance_id).
    async fn vrchat_get_current_instance(&self, account_name: &str) -> Result<VRChatInstanceBasic, Error>;
}
