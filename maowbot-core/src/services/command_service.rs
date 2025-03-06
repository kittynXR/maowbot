use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{Utc, DateTime};
use uuid::Uuid;
use tracing::{debug, info, warn, error};
use crate::Error;
use crate::models::{Command, CommandUsage, User};
use crate::repositories::{
    CommandRepository,
    CommandUsageRepository,
    CredentialsRepository,
    BotConfigRepository,
};
use crate::repositories::postgres::user::UserRepo;
use crate::services::builtin_commands::handle_builtin_command;
use crate::services::user_service::UserService;

/// Context passed to built-in command handlers.
pub struct CommandContext<'a> {
    pub channel: &'a str,
    pub user_roles: &'a [String],
    pub is_stream_online: bool,
    pub user_service: &'a Arc<UserService>,

    /// If the command was configured to respond using a specific credential ID,
    /// we pass that in here so the built-in logic can identify it if needed.
    pub respond_credential_id: Option<Uuid>,

    /// The user_name field from that credential, if found.
    pub respond_credential_name: Option<String>,

    pub credentials_repo: &'a Arc<dyn CredentialsRepository + Send + Sync>,

    /// **NEW**: So that builtin commands can read or write certain config keys
    /// (e.g. `vrchat_active_account`).
    pub bot_config_repo: &'a Arc<dyn BotConfigRepository + Send + Sync>,
}

/// (Updated) Response from command handlers. We now allow multiple lines to facilitate
/// multi-message output.
#[derive(Debug, Clone)]
pub struct CommandResponse {
    /// The lines to send as separate messages in chat.
    pub texts: Vec<String>,

    pub respond_credential_id: Option<Uuid>,
    pub platform: String,
    pub channel: String,
}

/// Tracks command cooldowns.
#[derive(Debug, Default)]
pub struct CooldownTracker {
    last_global_use: HashMap<Uuid, DateTime<Utc>>,
}

pub struct CommandService {
    command_repo: Arc<dyn CommandRepository + Send + Sync>,
    usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
    pub credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    pub user_service: Arc<UserService>,
    cooldowns: Arc<Mutex<CooldownTracker>>,

    /// **NEW**: So we can pass this to CommandContext easily.
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
}

impl CommandService {
    pub fn new(
        command_repo: Arc<dyn CommandRepository + Send + Sync>,
        usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
        user_service: Arc<UserService>,
        bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    ) -> Self {
        debug!("Initializing CommandService");
        Self {
            command_repo,
            usage_repo,
            credentials_repo,
            user_service,
            cooldowns: Arc::new(Mutex::new(CooldownTracker::default())),
            bot_config_repo,
        }
    }

    /// Processes a chat message and returns a command response if applicable.
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

        // 1) Verify message starts with '!'
        if !message_text.trim().starts_with('!') {
            return Ok(None);
        }

        // 2) Parse command and arguments
        let parts: Vec<&str> = message_text.trim().split_whitespace().collect();
        let cmd_part = parts[0].trim_start_matches('!');
        let args = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            String::new()
        };
        debug!("Parsed command: '{}', args: '{}'", cmd_part, args);

        // 3) Look up command in DB
        let cmd_opt = self.command_repo.get_command_by_name(platform, cmd_part).await?;
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

        // 4) Verify user role
        if cmd.min_role.to_lowercase() != "everyone" {
            let needed = cmd.min_role.to_lowercase();
            let has_role = user_roles.iter().any(|r| r.to_lowercase() == needed);
            if !has_role {
                debug!("User lacks required role '{}' for '{}'", needed, cmd.command_name);
                return Ok(Some(CommandResponse {
                    texts: vec![format!("You do not have permission to use {}.", cmd.command_name)],
                    respond_credential_id: cmd.respond_with_credential,
                    platform: cmd.platform.clone(),
                    channel: channel.to_string(),
                }));
            }
        }

        // 5) Check stream online/offline restrictions
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

        // 6) Enforce global cooldown
        let now = Utc::now();
        {
            let mut cd_lock = self.cooldowns.lock().unwrap();
            if let Some(last_time) = cd_lock.last_global_use.get(&cmd.command_id) {
                let elapsed = now.signed_duration_since(*last_time).num_seconds();
                let remain = cmd.cooldown_seconds as i64 - elapsed;
                if remain > 0 {
                    return Ok(Some(CommandResponse {
                        texts: vec![format!("Command {} is on cooldown. Please wait {}s.", cmd.command_name, remain)],
                        respond_credential_id: cmd.respond_with_credential,
                        platform: cmd.platform.clone(),
                        channel: channel.to_string(),
                    }));
                }
            }
            cd_lock.last_global_use.insert(cmd.command_id, now);
        }

        // 7) Log usage
        let usage = CommandUsage {
            usage_id: Uuid::new_v4(),
            command_id: cmd.command_id,
            user_id,
            used_at: now,
            channel: channel.to_string(),
            usage_text: Some(args.clone()),
            metadata: None,
        };
        if let Err(e) = self.usage_repo.insert_usage(&usage).await {
            error!("Error logging command usage: {:?}", e);
        }

        // 8) Retrieve user from DB
        let user_opt = self.user_service.user_manager.user_repo.get(user_id).await?;
        let user = match user_opt {
            Some(u) => u,
            None => {
                warn!("User {} not found in DB, using fallback record.", user_id);
                User {
                    user_id,
                    global_username: None,
                    created_at: now,
                    last_seen: now,
                    is_active: true,
                }
            }
        };

        // 9) Build context
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

        // If respond_with_credential was set, try to load that credential to get user_name
        if let Some(cid) = cmd.respond_with_credential {
            if let Ok(Some(cred)) = self.credentials_repo.get_credential_by_id(cid).await {
                ctx.respond_credential_name = Some(cred.user_name.clone());
            }
        }

        // 10) Invoke built-in logic
        if let Some(response_str) = handle_builtin_command(&cmd, &ctx, &user, &args).await? {
            // Some commands might embed <SPLIT> or multiple lines
            let lines: Vec<String> = response_str
                .split("<SPLIT>")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            return Ok(Some(CommandResponse {
                texts: lines,
                respond_credential_id: cmd.respond_with_credential,
                platform: cmd.platform.clone(),
                channel: channel.to_string(),
            }));
        }

        // 11) If no built-in logic returned anything, we just show a default message
        Ok(Some(CommandResponse {
            texts: vec![format!("Command {} recognized, but no built-in logic implemented.", cmd.command_name)],
            respond_credential_id: cmd.respond_with_credential,
            platform: cmd.platform.clone(),
            channel: channel.to_string(),
        }))
    }

    // ----------------------------------------------------------------
    // Additional CRUD / management methods
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
        }
        Ok(())
    }

    pub async fn set_command_active(&self, command_id: Uuid, is_active: bool) -> Result<(), Error> {
        if let Some(mut cmd) = self.command_repo.get_command_by_id(command_id).await? {
            cmd.is_active = is_active;
            cmd.updated_at = Utc::now();
            self.command_repo.update_command(&cmd).await?;
        }
        Ok(())
    }

    pub async fn delete_command(&self, command_id: Uuid) -> Result<(), Error> {
        self.command_repo.delete_command(command_id).await
    }

    pub async fn update_full_command(&self, cmd: &Command) -> Result<(), Error> {
        let mut to_save = cmd.clone();
        to_save.updated_at = Utc::now();
        self.command_repo.update_command(&to_save).await
    }
}
