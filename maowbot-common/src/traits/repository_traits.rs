use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::types::JsonValue;
use uuid::Uuid;
use crate::error::Error;
use crate::models::{Command, CommandUsage, Redeem, RedeemUsage, UserAnalysis};
use crate::models::discord::{DiscordAccountRecord, DiscordChannelRecord, DiscordGuildRecord, DiscordLiveRoleRecord};
use crate::models::link_request::LinkRequest;
use crate::models::platform::{Platform, PlatformConfig, PlatformCredential, PlatformIdentity};
use crate::models::user::{User, UserAuditLogEntry};
use crate::models::ai::{
    AiProvider, AiCredential, AiModel, AiTrigger, AiMemory, AiConfiguration, 
    AiTriggerWithDetails, AiAgent, AiAction, AiSystemPrompt, AiAgentWithDetails
};

#[async_trait]
pub trait Repository<T> {
    async fn create(&self, item: &T) -> Result<(), Error>;
    async fn get(&self, id: &str) -> Result<Option<T>, Error>;
    async fn update(&self, item: &T) -> Result<(), Error>;
    async fn delete(&self, id: &str) -> Result<(), Error>;
}

/// If you still keep a dedicated trait for platform identities:
#[async_trait]
pub trait PlatformIdentityRepository {
    // Possibly you want to use Uuid instead of &str for user_id as well
    // but if your code references it as &str, you can keep this shape.
    // The actual implementation can parse or store it as needed.
    fn get_by_platform(
        &self,
        platform: Platform,
        platform_user_id: &str
    ) -> impl std::future::Future<Output = Result<Option<PlatformIdentity>, Error>> + Send;

    fn get_all_for_user(
        &self,
        user_id: &str
    ) -> impl std::future::Future<Output = Result<Vec<PlatformIdentity>, Error>> + Send;
}

/// Repository trait for managing AI providers
#[async_trait]
pub trait AiProviderRepository: Send + Sync {
    async fn create_provider(&self, provider: &AiProvider) -> Result<(), Error>;
    async fn get_provider(&self, provider_id: Uuid) -> Result<Option<AiProvider>, Error>;
    async fn get_provider_by_name(&self, name: &str) -> Result<Option<AiProvider>, Error>;
    async fn list_providers(&self) -> Result<Vec<AiProvider>, Error>;
    async fn update_provider(&self, provider: &AiProvider) -> Result<(), Error>;
    async fn delete_provider(&self, provider_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing AI credentials
#[async_trait]
pub trait AiCredentialRepository: Send + Sync {
    async fn create_credential(&self, credential: &AiCredential) -> Result<(), Error>;
    async fn get_credential(&self, credential_id: Uuid) -> Result<Option<AiCredential>, Error>;
    async fn list_credentials_for_provider(&self, provider_id: Uuid) -> Result<Vec<AiCredential>, Error>;
    /// List all credentials across all providers
    async fn list_credentials(&self) -> Result<Vec<AiCredential>, Error>;
    async fn get_default_credential_for_provider(&self, provider_id: Uuid) -> Result<Option<AiCredential>, Error>;
    async fn update_credential(&self, credential: &AiCredential) -> Result<(), Error>;
    async fn set_default_credential(&self, credential_id: Uuid) -> Result<(), Error>;
    async fn delete_credential(&self, credential_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing AI models
#[async_trait]
pub trait AiModelRepository: Send + Sync {
    async fn create_model(&self, model: &AiModel) -> Result<(), Error>;
    async fn get_model(&self, model_id: Uuid) -> Result<Option<AiModel>, Error>;
    async fn get_model_by_name(&self, provider_id: Uuid, name: &str) -> Result<Option<AiModel>, Error>;
    async fn list_models_for_provider(&self, provider_id: Uuid) -> Result<Vec<AiModel>, Error>;
    async fn get_default_model_for_provider(&self, provider_id: Uuid) -> Result<Option<AiModel>, Error>;
    async fn update_model(&self, model: &AiModel) -> Result<(), Error>;
    async fn set_default_model(&self, model_id: Uuid) -> Result<(), Error>;
    async fn delete_model(&self, model_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing AI agents (MCPs)
#[async_trait]
pub trait AiAgentRepository: Send + Sync {
    async fn create_agent(&self, agent: &AiAgent) -> Result<(), Error>;
    async fn get_agent(&self, agent_id: Uuid) -> Result<Option<AiAgent>, Error>;
    async fn get_agent_by_name(&self, name: &str) -> Result<Option<AiAgent>, Error>;
    async fn list_agents(&self) -> Result<Vec<AiAgent>, Error>;
    async fn get_agent_with_details(&self, agent_id: Uuid) -> Result<Option<AiAgentWithDetails>, Error>;
    async fn update_agent(&self, agent: &AiAgent) -> Result<(), Error>;
    async fn delete_agent(&self, agent_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing AI actions
#[async_trait]
pub trait AiActionRepository: Send + Sync {
    async fn create_action(&self, action: &AiAction) -> Result<(), Error>;
    async fn get_action(&self, action_id: Uuid) -> Result<Option<AiAction>, Error>;
    async fn get_action_by_name(&self, agent_id: Uuid, name: &str) -> Result<Option<AiAction>, Error>;
    async fn list_actions_for_agent(&self, agent_id: Uuid) -> Result<Vec<AiAction>, Error>;
    async fn update_action(&self, action: &AiAction) -> Result<(), Error>;
    async fn delete_action(&self, action_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing AI system prompts
#[async_trait]
pub trait AiSystemPromptRepository: Send + Sync {
    async fn create_prompt(&self, prompt: &AiSystemPrompt) -> Result<(), Error>;
    async fn get_prompt(&self, prompt_id: Uuid) -> Result<Option<AiSystemPrompt>, Error>;
    async fn get_prompt_by_name(&self, name: &str) -> Result<Option<AiSystemPrompt>, Error>;
    async fn get_default_prompt(&self) -> Result<Option<AiSystemPrompt>, Error>;
    async fn list_prompts(&self) -> Result<Vec<AiSystemPrompt>, Error>;
    async fn update_prompt(&self, prompt: &AiSystemPrompt) -> Result<(), Error>;
    async fn set_default_prompt(&self, prompt_id: Uuid) -> Result<(), Error>;
    async fn delete_prompt(&self, prompt_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing AI triggers
#[async_trait]
pub trait AiTriggerRepository: Send + Sync {
    async fn create_trigger(&self, trigger: &AiTrigger) -> Result<(), Error>;
    async fn get_trigger(&self, trigger_id: Uuid) -> Result<Option<AiTrigger>, Error>;
    async fn get_trigger_by_pattern(&self, pattern: &str) -> Result<Option<AiTrigger>, Error>;
    async fn list_triggers(&self) -> Result<Vec<AiTrigger>, Error>;
    async fn list_triggers_for_model(&self, model_id: Uuid) -> Result<Vec<AiTrigger>, Error>;
    async fn list_triggers_for_agent(&self, agent_id: Uuid) -> Result<Vec<AiTrigger>, Error>;
    async fn list_triggers_with_details(&self) -> Result<Vec<AiTriggerWithDetails>, Error>;
    async fn update_trigger(&self, trigger: &AiTrigger) -> Result<(), Error>;
    async fn delete_trigger(&self, trigger_id: Uuid) -> Result<(), Error>;
}

/// Repository trait for managing AI memory
#[async_trait]
pub trait AiMemoryRepository: Send + Sync {
    async fn create_memory(&self, memory: &AiMemory) -> Result<(), Error>;
    async fn get_memory(&self, memory_id: Uuid) -> Result<Option<AiMemory>, Error>;
    async fn list_memories_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<AiMemory>, Error>;
    async fn delete_memory(&self, memory_id: Uuid) -> Result<(), Error>;
    async fn delete_user_memories(&self, user_id: Uuid) -> Result<(), Error>;
    async fn delete_old_memories(&self, older_than: DateTime<Utc>) -> Result<i64, Error>;
}

/// Repository trait for retrieving AI configurations
#[async_trait]
pub trait AiConfigurationRepository: Send + Sync {
    async fn get_default_configuration(&self) -> Result<Option<AiConfiguration>, Error>;
    async fn get_configuration_for_provider(&self, provider_name: &str) -> Result<Option<AiConfiguration>, Error>;
}


#[async_trait]
pub trait AnalyticsRepo: Send + Sync {
    async fn insert_chat_message(&self, msg: &crate::models::analytics::ChatMessage) -> Result<(), Error>;
    async fn insert_chat_messages(&self, msgs: &[crate::models::analytics::ChatMessage]) -> Result<(), Error>;

    async fn get_recent_messages(
        &self,
        platform: &str,
        channel: &str,
        limit: i64
    ) -> Result<Vec<crate::models::analytics::ChatMessage>, Error>;

    async fn insert_chat_session(&self, session: &crate::models::analytics::ChatSession) -> Result<(), Error>;
    async fn close_chat_session(
        &self,
        session_id: Uuid,
        left_at: DateTime<Utc>,
        duration_seconds: i64
    ) -> Result<(), Error>;

    async fn insert_bot_event(&self, event: &crate::models::analytics::BotEvent) -> Result<(), Error>;
    async fn update_daily_stats(
        &self,
        date_str: &str,
        new_messages: i64,
        new_visits: i64
    ) -> Result<(), Error>;

    async fn get_messages_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        maybe_platform: Option<&str>,
        maybe_channel: Option<&str>,
        maybe_search: Option<&str>,
    ) -> Result<Vec<crate::models::analytics::ChatMessage>, Error>;

    async fn reassign_user_messages(
        &self,
        from_user: Uuid,
        to_user: Uuid
    ) -> Result<u64, Error>;
}

#[async_trait]
pub trait BotConfigRepository: Send + Sync {
    async fn get_callback_port(&self) -> Result<Option<u16>, Error>;
    async fn set_callback_port(&self, port: u16) -> Result<(), Error>;
    async fn set_value(&self, config_key: &str, config_value: &str) -> Result<(), Error>;
    async fn get_value(&self, config_key: &str) -> Result<Option<String>, Error>;

    // NEW:
    async fn get_autostart(&self) -> Result<Option<String>, Error> {
        self.get_value("autostart").await
    }
    async fn set_autostart(&self, json_str: &str) -> Result<(), Error> {
        self.set_value("autostart", json_str).await
    }
    async fn list_all(&self) -> Result<Vec<(String, String)>, Error>;
    /// NEW: Delete a row by config_key
    async fn delete_value(&self, config_key: &str) -> Result<(), Error>;
    async fn delete_value_kv(&self, config_key: &str, config_value: &str) -> Result<(), Error>;
    async fn get_value_kv_meta(
        &self,
        config_key: &str,
        config_value: &str
    ) -> Result<Option<(String, Option<JsonValue>)>, Error>;
    async fn set_value_kv_meta(
        &self,
        config_key: &str,
        config_value: &str,
        config_meta: Option<JsonValue>
    ) -> Result<(), Error>;
}

#[async_trait]
pub trait CommandUsageRepository: Send + Sync {
    async fn insert_usage(&self, usage: &CommandUsage) -> Result<(), Error>;
    async fn list_usage_for_command(&self, command_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
    async fn list_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<CommandUsage>, Error>;
}

#[async_trait]
pub trait CommandRepository: Send + Sync {
    async fn create_command(&self, cmd: &Command) -> Result<(), Error>;
    async fn get_command_by_id(&self, command_id: Uuid) -> Result<Option<Command>, Error>;
    async fn get_command_by_name(&self, platform: &str, command_name: &str) -> Result<Option<Command>, Error>;
    async fn list_commands(&self, platform: &str) -> Result<Vec<Command>, Error>;
    async fn update_command(&self, cmd: &Command) -> Result<(), Error>;
    async fn delete_command(&self, command_id: Uuid) -> Result<(), Error>;
}

#[async_trait]
pub trait CredentialsRepository: Send + Sync {
    async fn store_credentials(&self, creds: &PlatformCredential) -> Result<(), Error>;
    async fn get_credentials(&self, platform: &Platform, user_id: Uuid) -> Result<Option<PlatformCredential>, Error>;
    async fn get_credential_by_id(&self, credential_id: Uuid) -> Result<Option<PlatformCredential>, Error>;
    async fn update_credentials(&self, creds: &PlatformCredential) -> Result<(), Error>;
    async fn delete_credentials(&self, platform: &Platform, user_id: Uuid) -> Result<(), Error>;
    async fn get_expiring_credentials(&self, within: Duration) -> Result<Vec<PlatformCredential>, Error>;
    async fn get_all_credentials(&self) -> Result<Vec<PlatformCredential>, Error>;
    async fn list_credentials_for_platform(&self, platform: &Platform) -> Result<Vec<PlatformCredential>, Error>;
    async fn get_broadcaster_credential(&self, platform: &Platform) -> Result<Option<PlatformCredential>, Error>;
    async fn get_bot_credentials(&self, platform: &Platform) -> Result<Option<PlatformCredential>, Error>;
    async fn get_teammate_credentials(&self, platform: &Platform) -> Result<Vec<PlatformCredential>, Error>;
}

#[async_trait]
pub trait LinkRequestsRepository {
    async fn create_link_request(&self, req: &LinkRequest) -> Result<(), Error>;
    async fn get_link_request(&self, link_request_id: Uuid) -> Result<Option<LinkRequest>, Error>;
    async fn update_link_request(&self, req: &LinkRequest) -> Result<(), Error>;
    async fn delete_link_request(&self, link_request_id: Uuid) -> Result<(), Error>;
}

#[async_trait]
pub trait PlatformConfigRepository: Send + Sync {
    async fn upsert_platform_config(
        &self,
        platform: &str,
        client_id: Option<String>,
        client_secret: Option<String>,
    ) -> Result<(), Error>;

    async fn get_platform_config(&self, platform_config_id: Uuid) -> Result<Option<PlatformConfig>, Error>;
    async fn list_platform_configs(&self, maybe_platform: Option<&str>) -> Result<Vec<PlatformConfig>, Error>;
    async fn delete_platform_config(&self, platform_config_id: Uuid) -> Result<(), Error>;
    async fn get_by_platform(&self, platform: &str) -> Result<Option<PlatformConfig>, Error>;
    async fn count_for_platform(&self, platform: &str) -> Result<i64, Error>;
}


#[async_trait]
pub trait PlatformIdentityRepo {
    async fn create(&self, identity: &PlatformIdentity) -> Result<(), Error>;
    async fn get(&self, id: Uuid) -> Result<Option<PlatformIdentity>, Error>;
    async fn update(&self, identity: &PlatformIdentity) -> Result<(), Error>;
    async fn delete(&self, id: Uuid) -> Result<(), Error>;

    async fn get_by_platform(
        &self,
        platform: Platform,
        platform_user_id: &str
    ) -> Result<Option<PlatformIdentity>, Error>;

    async fn get_all_for_user(&self, user_id: Uuid)
                              -> Result<Vec<PlatformIdentity>, Error>;

    async fn get_by_user_and_platform(
        &self,
        user_id: Uuid,
        platform: &Platform,
    ) -> Result<Option<PlatformIdentity>, Error>;
}

#[async_trait]
pub trait RedeemUsageRepository: Send + Sync {
    async fn insert_usage(&self, usage: &RedeemUsage) -> Result<(), Error>;
    async fn list_usage_for_redeem(&self, redeem_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn list_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
}

#[async_trait]
pub trait RedeemRepository: Send + Sync {
    async fn create_redeem(&self, rd: &Redeem) -> Result<(), Error>;
    async fn get_redeem_by_id(&self, redeem_id: Uuid) -> Result<Option<Redeem>, Error>;
    async fn get_redeem_by_reward_id(&self, platform: &str, reward_id: &str) -> Result<Option<Redeem>, Error>;
    async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error>;
    async fn update_redeem(&self, rd: &Redeem) -> Result<(), Error>;
    async fn delete_redeem(&self, redeem_id: Uuid) -> Result<(), Error>;
}

#[async_trait::async_trait]
pub trait UserRepo {
    async fn create(&self, user: &User) -> Result<(), Error>;
    async fn get(&self, id: Uuid) -> Result<Option<User>, Error>;
    async fn get_by_global_username(&self, name: &str) -> Result<Option<User>, Error>;
    async fn update(&self, user: &User) -> Result<(), Error>;
    async fn delete(&self, id: Uuid) -> Result<(), Error>;
    async fn list_all(&self) -> Result<Vec<User>, Error>;
}

#[async_trait]
pub trait UserAnalysisRepository: Send + Sync {
    async fn create_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error>;
    async fn get_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error>;
    async fn update_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error>;
}

#[async_trait]
pub trait UserAuditLogRepository {
    async fn insert_entry(&self, entry: &UserAuditLogEntry) -> Result<(), Error>;
    async fn get_entry(&self, audit_id: Uuid) -> Result<Option<UserAuditLogEntry>, Error>;
    async fn get_entries_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<UserAuditLogEntry>, Error>;
}

#[async_trait]
pub trait DiscordRepository {
    // Existing methods for guilds/channels:
    async fn upsert_guild(&self, account_name: &str, guild_id: &str, guild_name: &str) -> Result<(), Error>;
    async fn list_guilds_for_account(&self, account_name: &str) -> Result<Vec<DiscordGuildRecord>, Error>;
    async fn get_guild(&self, account_name: &str, guild_id: &str) -> Result<Option<DiscordGuildRecord>, Error>;

    async fn upsert_channel(&self,
                            account_name: &str,
                            guild_id: &str,
                            channel_id: &str,
                            channel_name: &str
    ) -> Result<(), Error>;
    async fn list_channels_for_guild(&self,
                                     account_name: &str,
                                     guild_id: &str
    ) -> Result<Vec<DiscordChannelRecord>, Error>;

    // "Active server" was previously done in a separate table, now we rely on is_active
    async fn set_active_server(&self, account_name: &str, guild_id: &str) -> Result<(), Error>;
    async fn get_active_server(&self, account_name: &str) -> Result<Option<String>, Error>;

    // New for "active account", "active channel", and listing accounts:
    async fn list_accounts(&self) -> Result<Vec<DiscordAccountRecord>, Error>;
    async fn upsert_account(&self, account_name: &str, maybe_credential: Option<Uuid>, discord_id: Option<&str>) -> Result<(), Error>;
    async fn set_active_account(&self, account_name: &str) -> Result<(), Error>;
    async fn get_active_account(&self) -> Result<Option<String>, Error>;

    async fn set_active_channel(&self, account_name: &str, guild_id: &str, channel_id: &str) -> Result<(), Error>;
    async fn get_active_channel(&self, account_name: &str, guild_id: &str) -> Result<Option<String>, Error>;
    
    // Live role methods for Twitch streamers
    async fn set_live_role(&self, guild_id: &str, role_id: &str) -> Result<(), Error>;
    async fn get_live_role(&self, guild_id: &str) -> Result<Option<DiscordLiveRoleRecord>, Error>;
    async fn delete_live_role(&self, guild_id: &str) -> Result<(), Error>;
    async fn list_live_roles(&self) -> Result<Vec<DiscordLiveRoleRecord>, Error>;
}

/// Repository trait for managing OBS instances
#[async_trait]
pub trait ObsRepository: Send + Sync {
    async fn get_instance(&self, instance_number: u32) -> Result<Option<maowbot_obs::ObsInstance>, Error>;
    async fn update_instance(&self, instance: &maowbot_obs::ObsInstance) -> Result<(), Error>;
    async fn set_connection_status(&self, instance_number: u32, connected: bool) -> Result<(), Error>;
    async fn list_instances(&self) -> Result<Vec<maowbot_obs::ObsInstance>, Error>;
    async fn get_connection_info(&self, instance_number: u32) -> Result<Option<(bool, Option<DateTime<Utc>>)>, Error>;
}