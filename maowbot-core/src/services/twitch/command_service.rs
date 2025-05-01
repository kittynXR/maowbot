use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{Utc, DateTime};
use uuid::Uuid;
use tracing::{debug, warn, error};
use maowbot_common::models::{Command, CommandUsage};
use maowbot_common::models::platform::Platform::TwitchIRC;
use maowbot_common::models::user::User;
use maowbot_common::traits::repository_traits::{
    BotConfigRepository,
    CommandRepository,
    CommandUsageRepository,
    CredentialsRepository,
    UserRepo
};
use maowbot_common::models::platform::PlatformCredential;
use crate::Error;
use crate::services::twitch::builtin_commands::handle_builtin_command;
use crate::services::user_service::UserService;
use crate::services::message_sender::{MessageSender, MessageResponse};

/// Context passed to built-in command handlers.
pub struct CommandContext<'a> {
    pub channel: &'a str,
    pub user_roles: &'a [String],
    pub is_stream_online: bool,
    pub user_service: &'a Arc<UserService>,
    pub respond_credential_id: Option<Uuid>,
    pub respond_credential_name: Option<String>,

    pub credentials_repo: &'a Arc<dyn CredentialsRepository + Send + Sync>,
    pub bot_config_repo: &'a Arc<dyn BotConfigRepository + Send + Sync>,
}

/// Response from command handlers: multiple lines + which credential we used + which channel.
/// This is now just a type alias for the shared MessageResponse type
pub type CommandResponse = MessageResponse;

/// Tracks command cooldowns globally, etc.
#[derive(Debug, Default)]
pub struct CooldownTracker {
    last_global_use: HashMap<Uuid, DateTime<Utc>>,
}

/// The main service for handling custom commands, building on a commands-cache in memory.
pub struct CommandService {
    command_repo: Arc<dyn CommandRepository + Send + Sync>,
    usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
    pub credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    pub user_service: Arc<UserService>,
    cooldowns: Arc<Mutex<CooldownTracker>>,

    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    
    // Platform manager reference for sending messages
    pub platform_manager: Arc<crate::platforms::manager::PlatformManager>,
    
    // Message sender for handling outgoing messages
    pub message_sender: MessageSender,

    // ----------------------------------------------------------------
    // NEW: an in-memory cache of commands, loaded once at startup or
    // after any changes. We avoid re-querying the DB on every message.
    // ----------------------------------------------------------------
    commands_cache: Arc<Mutex<HashMap<String, Command>>>,
}

impl CommandService {
    pub fn new(
        command_repo: Arc<dyn CommandRepository + Send + Sync>,
        usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
        user_service: Arc<UserService>,
        bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
        platform_manager: Arc<crate::platforms::manager::PlatformManager>,
    ) -> Self {
        debug!("Initializing CommandService");
        
        // Create MessageSender instance
        let message_sender = MessageSender::new(
            credentials_repo.clone(),
            platform_manager.clone()
        );

        let svc = Self {
            command_repo,
            usage_repo,
            credentials_repo,
            user_service,
            cooldowns: Arc::new(Mutex::new(CooldownTracker::default())),
            bot_config_repo,
            platform_manager,
            message_sender,
            commands_cache: Arc::new(Mutex::new(HashMap::new())),
        };

        // On creation, load all commands from DB into memory:
        svc.reload_commands_cache();
        svc
    }

    /// Re-fetch all commands from the DB and store them in our local HashMap,
    /// keyed by `(platform.to_lowercase(), command_name.to_lowercase())`.
    pub fn reload_commands_cache(&self) {
        let command_repo = self.command_repo.clone();
        let mut cache_guard = self.commands_cache.lock().unwrap();
        cache_guard.clear();

        // We handle multiple platforms, so let's do a quick gather:
        // (In practice you might call list_commands for each platform or fetch all at once.)
        let platforms = ["twitch-irc", "twitch", "discord"]; // etc.
        for &pf in &platforms {
            match futures_lite::future::block_on(command_repo.list_commands(pf)) {
                Ok(cmds) => {
                    for c in cmds {
                        let key = format!("{}|{}", c.platform.to_lowercase(), c.command_name.to_lowercase());
                        cache_guard.insert(key, c);
                    }
                }
                Err(e) => {
                    error!("Error loading commands for {} => {:?}", pf, e);
                }
            }
        }

        debug!("reload_commands_cache => loaded {} commands total", cache_guard.len());
    }

    /// Lookup a command from our in-memory cache by (platform, command_name).
    fn find_command_in_cache(&self, platform: &str, command_name: &str) -> Option<Command> {
        let key = format!("{}|{}", platform.to_lowercase(), command_name.to_lowercase());
        let lock = self.commands_cache.lock().unwrap();
        lock.get(&key).cloned()
    }

    /// Processes a chat message and returns a command response if we find a matching “!command”.
    pub async fn handle_chat_line(
        &self,
        platform: &str,
        channel: &str,
        user_id: Uuid,
        user_roles: &[String],
        message_text: &str,
        is_stream_online: bool,
    ) -> Result<Option<CommandResponse>, Error> {
        debug!("handle_chat_line() received message: '{}'", message_text);

        // -----------------------------------------------------------------
        // 1) Must start with '!'
        // -----------------------------------------------------------------
        if !message_text.trim().starts_with('!') {
            return Ok(None);
        }
        let parts: Vec<&str> = message_text.trim().split_whitespace().collect();
        let cmd_part = parts[0].trim_start_matches('!');
        let args = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            String::new()
        };

        // -----------------------------------------------------------------
        // 2) Built-in meta commands (handled without DB)
        // -----------------------------------------------------------------
        match cmd_part.to_lowercase().as_str() {
            "continue" => {
                let sent = self
                    .message_sender
                    .handle_continue_command(channel, None, user_id)
                    .await?;

                if !sent {
                    self.message_sender
                        .send_twitch_message(channel, "No continuation available.", None, user_id)
                        .await
                        .ok();
                }
                return Ok(None);
            }
            "sources" => {
                let sent = self
                    .message_sender
                    .handle_sources_command(channel, None, user_id)
                    .await?;

                if !sent {
                    self.message_sender
                        .send_twitch_message(
                            channel,
                            "No recent AI message found. Sources not available.",
                            None,
                            user_id,
                        )
                        .await
                        .ok();
                }
                return Ok(None);
            }
            _ => { /* fall through to DB commands */ }
        }

        // -----------------------------------------------------------------
        // 3) Look up command in cache / DB
        // -----------------------------------------------------------------
        let cmd_opt = self.find_command_in_cache(platform, cmd_part);
        let cmd = match cmd_opt {
            Some(c) => c,
            None => {
                debug!("No command found matching '{}'", cmd_part);
                return Ok(None);
            }
        };
        if !cmd.is_active {
            debug!("Command '{}' is inactive.", cmd.command_name);
            return Ok(None);
        }

        // 3) Check roles
        if cmd.min_role.to_lowercase() != "everyone" {
            let needed = cmd.min_role.to_lowercase();
            let has_role = user_roles.iter().any(|r| r.to_lowercase() == needed);
            if !has_role {
                return Ok(Some(CommandResponse {
                    texts: vec![format!("You lack the required role '{}' to use this.", cmd.min_role)],
                    respond_credential_id: cmd.respond_with_credential,
                    platform: cmd.platform.clone(),
                    channel: channel.to_string(),
                }));
            }
        }

        // 4) Stream constraints
        if cmd.stream_online_only && !is_stream_online {
            return Ok(Some(CommandResponse {
                texts: vec![format!("Command {} can only be used when stream is online.", cmd.command_name)],
                respond_credential_id: cmd.respond_with_credential,
                platform: cmd.platform.clone(),
                channel: channel.to_string(),
            }));
        }
        if cmd.stream_offline_only && is_stream_online {
            return Ok(Some(CommandResponse {
                texts: vec![format!("Command {} can only be used when stream is offline.", cmd.command_name)],
                respond_credential_id: cmd.respond_with_credential,
                platform: cmd.platform.clone(),
                channel: channel.to_string(),
            }));
        }

        // 5) Check cooldown
        let now = Utc::now();
        {
            let mut cd_lock = self.cooldowns.lock().unwrap();
            if let Some(last_time) = cd_lock.last_global_use.get(&cmd.command_id) {
                let elapsed = now.signed_duration_since(*last_time).num_seconds();
                let remain = cmd.cooldown_seconds as i64 - elapsed;
                if remain > 0 {
                    return Ok(Some(CommandResponse {
                        texts: vec![format!("Command {} is on cooldown. Wait {}s.", cmd.command_name, remain)],
                        respond_credential_id: cmd.respond_with_credential,
                        platform: cmd.platform.clone(),
                        channel: channel.to_string(),
                    }));
                }
            }
            cd_lock.last_global_use.insert(cmd.command_id, now);
        }

        // 6) Insert usage
        let usage = CommandUsage {
            usage_id: Uuid::new_v4(),
            command_id: cmd.command_id,
            user_id,
            used_at: now,
            channel: channel.to_string(),
            usage_text: args.clone(),
            metadata: None,
        };
        if let Err(e) = self.usage_repo.insert_usage(&usage).await {
            error!("Error logging command usage: {:?}", e);
        }

        // 7) Load user from DB
        let user_opt = self.user_service.user_manager.user_repo.get(user_id).await?;
        let user = user_opt.unwrap_or(User {
            user_id,
            global_username: None,
            created_at: now,
            last_seen: now,
            is_active: true,
        });

        // 8) Build context
        let mut ctx = CommandContext {
            channel,
            user_roles,
            is_stream_online,
            user_service: &self.user_service,
            respond_credential_id: cmd.respond_with_credential,
            respond_credential_name: None,
            credentials_repo: &self.credentials_repo,
            bot_config_repo: &self.bot_config_repo,
        };

        // If there's a respond_with_credential, see if we can load that credential’s user_name
        if let Some(cid) = cmd.respond_with_credential {
            if let Ok(Some(cred)) = self.credentials_repo.get_credential_by_id(cid).await {
                ctx.respond_credential_name = Some(cred.user_name.clone());
            }
        }

        // Check for special commands: !sources and !continue
        match cmd_part.to_lowercase().as_str() {
            "continue" => {
                let sent = self
                    .message_sender
                    .handle_continue_command(channel, None, user_id)
                    .await?;

                if !sent {
                    // Gracefully inform the user – quick inline response
                    self.message_sender
                        .send_twitch_message(
                            channel,
                            "No continuation available.",
                            None,
                            user_id,
                        )
                        .await
                        .ok();
                }
                return Ok(None); // Already handled
            }
            "sources" => {
                let sent = self
                    .message_sender
                    .handle_sources_command(channel, None, user_id)
                    .await?;

                if !sent {
                    self.message_sender
                        .send_twitch_message(
                            channel,
                            "No recent AI message found. Sources not available.",
                            None,
                            user_id,
                        )
                        .await
                        .ok();
                }
                return Ok(None);
            }
            _ => { /* fallthrough to DB-defined commands */ }
        }
        
        // 9) Check for built-in logic
        if let Some(response_str) = handle_builtin_command(&cmd, &ctx, &user, &args).await? {
            let lines: Vec<String> = response_str
                .split("<SPLIT>")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // *Now* figure out which credential we will respond *from*.
            let actual_respond_cred_id = self.pick_response_credential_id(&cmd, user_id).await?;
            return Ok(Some(CommandResponse {
                texts: lines,
                respond_credential_id: actual_respond_cred_id,
                platform: cmd.platform.clone(),
                channel: channel.to_string(),
            }));
        }

        // 10) No built-in logic => default text
        let actual_respond_cred_id = self.pick_response_credential_id(&cmd, user_id).await?;
        Ok(Some(CommandResponse {
            texts: vec![format!("Command {} recognized but no built-in logic found.", cmd.command_name)],
            respond_credential_id: actual_respond_cred_id,
            platform: cmd.platform.clone(),
            channel: channel.to_string(),
        }))
    }

    /// Determine which Twitch-IRC credential we should use to send the reply.
    ///
    /// The user’s new rules:
    ///  1) If `cmd.active_credential_id` is set and is a valid *bot* credential, use that.
    ///  2) If no such valid credential, we check if there's any bot at all: pick the first we find.
    ///  3) If no bot, pick a broadcaster (the first broadcaster we find).
    ///  4) If no broadcaster either, use the account that actually received this message (user_id).
    async fn pick_response_credential_id(
        &self,
        cmd: &Command,
        message_sender_user_id: Uuid
    ) -> Result<Option<Uuid>, Error> {
        // #1: if the command’s `active_credential_id` is set:
        if let Some(cid) = cmd.active_credential_id {
            if let Ok(Some(c)) = self.credentials_repo.get_credential_by_id(cid).await {
                if c.is_bot && c.platform == TwitchIRC {
                    return Ok(Some(cid));
                }
            }
        }

        // #2: find the first “bot” Twitch‑IRC credential
        let all_creds = self.credentials_repo.list_credentials_for_platform(&TwitchIRC).await?;
        if let Some(bot_cred) = all_creds.iter().find(|c| c.is_bot) {
            return Ok(Some(bot_cred.credential_id));
        }

        // #3: find the first broadcaster
        if let Some(broadcaster_cred) = all_creds.iter().find(|c| c.is_broadcaster) {
            return Ok(Some(broadcaster_cred.credential_id));
        }

        // #4: no bot, no broadcaster => use the same user’s own Twitch-IRC credential if it exists
        let maybe_same_user_cred = self.credentials_repo.get_credentials(
            &TwitchIRC,
            message_sender_user_id
        ).await?;
        if let Some(c) = maybe_same_user_cred {
            return Ok(Some(c.credential_id));
        }

        // If we can’t find *any* suitable credential, just return None.
        Ok(None)
    }

    // ----------------------------------------------------------------
    // Additional CRUD methods (unchanged)
    // ----------------------------------------------------------------

    pub async fn create_command(
        &self,
        platform: &str,
        command_name: &str,
        min_role: &str,
    ) -> Result<Command, Error> {
        debug!("Creating new command for platform '{}': '{}'", platform, command_name);
        let now = Utc::now();
        let cmd = Command {
            command_id: Uuid::new_v4(),
            active_credential_id: None,
            platform: platform.to_string(),
            command_name: command_name.to_string(),
            min_role: min_role.to_string(),
            is_active: true,
            created_at: now,
            updated_at: now,
            cooldown_seconds: 0,
            cooldown_warnonce: false,
            respond_with_credential: None,
            stream_online_only: false,
            stream_offline_only: false,
        };
        self.command_repo.create_command(&cmd).await?;
        // Also refresh in-memory:
        self.reload_commands_cache();
        Ok(cmd)
    }

    pub async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error> {
        self.command_repo.list_commands(platform).await
    }

    pub async fn update_command_role(&self, command_id: Uuid, new_role: &str) -> Result<(), Error> {
        if let Some(mut cmd) = self.command_repo.get_command_by_id(command_id).await? {
            cmd.min_role = new_role.to_string();
            cmd.updated_at = Utc::now();
            self.command_repo.update_command(&cmd).await?;
            self.reload_commands_cache();
        }
        Ok(())
    }

    pub async fn set_command_active(&self, command_id: Uuid, is_active: bool) -> Result<(), Error> {
        if let Some(mut cmd) = self.command_repo.get_command_by_id(command_id).await? {
            cmd.is_active = is_active;
            cmd.updated_at = Utc::now();
            self.command_repo.update_command(&cmd).await?;
            self.reload_commands_cache();
        }
        Ok(())
    }

    pub async fn delete_command(&self, command_id: Uuid) -> Result<(), Error> {
        self.command_repo.delete_command(command_id).await?;
        self.reload_commands_cache();
        Ok(())
    }

    pub async fn update_full_command(&self, cmd: &Command) -> Result<(), Error> {
        let mut to_save = cmd.clone();
        to_save.updated_at = Utc::now();
        self.command_repo.update_command(&to_save).await?;
        self.reload_commands_cache();
        Ok(())
    }
}
