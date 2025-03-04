use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{Utc, DateTime};
use uuid::Uuid;
use tracing::{info, warn, error};

use crate::Error;
use crate::models::{Command, CommandUsage, User};
use crate::repositories::{
    CommandRepository, CommandUsageRepository,
};
use crate::repositories::postgres::user::UserRepo;
use crate::services::builtin_commands::handle_builtin_command;
use crate::services::user_service::UserService;

/// A context object provided to built-in command handlers.
/// Includes references to needed services, info about the channel, user roles, etc.
pub struct CommandContext<'a> {
    pub channel: &'a str,
    pub user_roles: &'a [String],
    /// If we know the stream is currently live or not.
    /// (In real usage, you might track this with a Twitch event or a streaming API check.)
    pub is_stream_online: bool,

    /// For advanced usage, we can pass references to the user_service or other managers:
    pub user_service: &'a Arc<UserService>,

    /// If the command is configured to respond from a particular credential,
    /// we can look up the "account name" now or later.
    pub respond_credential_id: Option<Uuid>,
    /// Possibly the resolved account name (like "MyBotAccount"), if we found it.
    pub respond_credential_name: Option<String>,
}

impl<'a> CommandContext<'a> {
    /// For convenience in builtin command logic, we get the final responding account name (if any).
    pub fn responding_account_name(&self) -> Option<&str> {
        self.respond_credential_name.as_deref()
    }
}

/// A structure to track cooldown states in memory.
/// Key = (command_id, user_id, or maybe just command_id?), Value = time last used + optional warnings.
#[derive(Debug, Default)]
pub struct CooldownTracker {
    /// For simplicity: store the last time the command was used.
    /// Key = command_id, Val = datetime of last usage (UTC).
    last_global_use: HashMap<Uuid, DateTime<Utc>>,

    // If we wanted per-user cooldown, we’d store a key=(command_id, user_id).
}

/// The CommandService processes “bang” commands like `!ping`.
/// It loads the command from the DB, checks roles, checks cooldown,
/// logs usage, and either calls our built-in logic or does something else.
pub struct CommandService {
    command_repo: Arc<dyn CommandRepository + Send + Sync>,
    usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
    user_service: Arc<UserService>,

    cooldowns: Arc<Mutex<CooldownTracker>>,
}

impl CommandService {
    pub fn new(
        command_repo: Arc<dyn CommandRepository + Send + Sync>,
        usage_repo: Arc<dyn CommandUsageRepository + Send + Sync>,
        user_service: Arc<UserService>,
    ) -> Self {
        Self {
            command_repo,
            usage_repo,
            user_service,
            cooldowns: Arc::new(Mutex::new(CooldownTracker::default())),
        }
    }

    /// Attempt to handle a chat line that might be a command.
    /// Return Ok(Some(response_text)) if it’s recognized and responded,
    /// Ok(None) if not a command, Err if something goes wrong.
    pub async fn handle_chat_line(
        &self,
        platform: &str,
        channel: &str,
        user_id: Uuid,
        user_roles: &[String],
        message_text: &str,
        is_stream_online: bool,
    ) -> Result<Option<String>, Error> {
        // 1) check if it starts with '!'
        if !message_text.trim().starts_with('!') {
            return Ok(None);
        }

        // parse out the command part and the "args" after the command
        let parts: Vec<&str> = message_text.trim().split_whitespace().collect();
        let cmd_part = parts[0].trim_start_matches('!');
        let args = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            "".to_string()
        };

        // 2) find the command in DB
        let cmd_opt = self.command_repo
            .get_command_by_name(platform, cmd_part)
            .await?;
        let cmd = match cmd_opt {
            Some(c) => c,
            None => {
                // unknown command
                return Ok(None);
            }
        };

        // 3) is_active?
        if !cmd.is_active {
            return Ok(None);
        }

        // 4) check min_role
        if cmd.min_role.to_lowercase() != "everyone" {
            let needed = cmd.min_role.to_lowercase();
            let has_role = user_roles
                .iter()
                .any(|r| r.to_lowercase() == needed);
            if !has_role {
                // lacking role
                return Ok(Some(format!("You do not have permission to use {}.", cmd.command_name)));
            }
        }

        // 5) check stream_online_only / offline_only vs is_stream_online
        if cmd.stream_online_only && !is_stream_online {
            // command restricted to live/online
            return Ok(Some(format!("Command {} can only be used when stream is online.", cmd.command_name)));
        }
        if cmd.stream_offline_only && is_stream_online {
            // command restricted to offline
            return Ok(Some(format!("Command {} can only be used when stream is offline.", cmd.command_name)));
        }

        // 6) check cooldown
        let now = Utc::now();
        if cmd.cooldown_seconds > 0 {
            let mut cooldown_lock = self.cooldowns.lock().unwrap();
            if let Some(last_time) = cooldown_lock.last_global_use.get(&cmd.command_id) {
                let elapsed = (now.signed_duration_since(*last_time)).num_seconds();
                let remain = cmd.cooldown_seconds as i64 - elapsed;
                if remain > 0 {
                    // we’re still on cooldown
                    if cmd.cooldown_warnonce {
                        // If “warnonce” is set, we only warn the FIRST time we see the user use it
                        // again during cooldown; subsequent attempts are silently ignored.
                        // For simplicity, we do the “first time” approach globally, not user-specific:
                        // We'll remove the cooldown entry so we won't warn again.
                        cooldown_lock.last_global_use.remove(&cmd.command_id);
                        return Ok(Some(format!(
                            "Command {} is on cooldown. Please wait {} seconds.",
                            cmd.command_name, remain
                        )));
                    } else {
                        // Warn every time:
                        return Ok(Some(format!(
                            "Command {} is on cooldown. Please wait {} seconds.",
                            cmd.command_name, remain
                        )));
                    }
                }
            }
            // not on cooldown => record usage time
            cooldown_lock.last_global_use.insert(cmd.command_id, now);
        }

        // 7) log usage
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
            error!("Failed to insert command usage => {:?}", e);
        }

        // 8) retrieve the user from DB for passing into builtin logic
        let user = match self.user_service.user_manager.user_repo.get(user_id).await? {
            Some(u) => u,
            None => {
                // fallback user
                let mut tmp = User {
                    user_id,
                    global_username: None,
                    created_at: now,
                    last_seen: now,
                    is_active: true,
                };
                tmp
            }
        };

        // 9) Build the context
        let respond_credential_id = cmd.respond_with_credential;
        // You might look up that credential to get a user-facing name:
        let respond_credential_name = None; // stub

        let ctx = CommandContext {
            channel,
            user_roles,
            is_stream_online,
            user_service: &self.user_service,
            respond_credential_id,
            respond_credential_name,
        };

        // 10) check if it’s one of our built-in commands:
        let maybe_builtin_response = handle_builtin_command(&cmd, &ctx, &user, &args).await?;
        if let Some(response) = maybe_builtin_response {
            // We handled it as a built-in command
            return Ok(Some(response));
        }

        // If we get here, it’s not recognized as a builtin =>
        // we might handle custom commands from the DB, or do something else.
        // For example, you might store a message template in the DB for each command.
        // We'll just respond with a generic placeholder for custom commands:
        Ok(Some(format!("Command {} recognized, but no built-in logic implemented.", cmd.command_name)))
    }

    // Additional service methods for command CRUD:
    pub async fn create_command(&self, platform: &str, command_name: &str, min_role: &str) -> Result<Command, Error> {
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
        to_save.updated_at = chrono::Utc::now();
        self.command_repo.update_command(&to_save).await
    }
}