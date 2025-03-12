use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;
use crate::error::Error;
use crate::models::{Command, CommandUsage, Redeem, RedeemUsage, UserAnalysis};
use crate::models::link_request::LinkRequest;
use crate::models::platform::{Platform, PlatformConfig, PlatformCredential, PlatformIdentity};
use crate::models::user::{User, UserAuditLogEntry};

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

    /// Returns the single credential for a specific `(platform, user_id)`, or `None`.
    async fn get_credentials(&self, platform: &Platform, user_id: Uuid) -> Result<Option<PlatformCredential>, Error>;

    /// Returns a single credential by credential_id, or `None`.
    async fn get_credential_by_id(&self, credential_id: Uuid) -> Result<Option<PlatformCredential>, Error>;

    async fn update_credentials(&self, creds: &PlatformCredential) -> Result<(), Error>;
    async fn delete_credentials(&self, platform: &Platform, user_id: Uuid) -> Result<(), Error>;

    /// Lists credentials expiring within a certain duration from now.
    async fn get_expiring_credentials(&self, within: Duration) -> Result<Vec<PlatformCredential>, Error>;

    /// Lists *all* credentials across all platforms.
    async fn get_all_credentials(&self) -> Result<Vec<PlatformCredential>, Error>;

    /// **NEW**: Returns all credentials for the specified platform, decryption included.
    async fn list_credentials_for_platform(&self, platform: &Platform) -> Result<Vec<PlatformCredential>, Error>;
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

