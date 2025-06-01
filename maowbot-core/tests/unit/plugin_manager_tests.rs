// File: maowbot-core/tests/unit/plugin_manager_tests.rs

use std::sync::Arc;
use tokio::sync::mpsc;
use maowbot_core::{
    plugins::manager::PluginManager,
    Error,
};
use maowbot_proto::plugs::{
    plugin_stream_request::Payload as ReqPayload,
    LogMessage, RequestStatus, RequestCaps,
    Hello, 
};

// We need to mock all the required dependencies for the PluginManager
use sqlx::postgres::PgPoolOptions;
use maowbot_core::repositories::postgres::user::UserRepository;
use maowbot_core::repositories::postgres::drip::DripRepository;
use maowbot_core::repositories::postgres::discord::PostgresDiscordRepository;
use maowbot_core::repositories::postgres::analytics::PostgresAnalyticsRepository;
use maowbot_core::repositories::postgres::user_analysis::PostgresUserAnalysisRepository;
use maowbot_core::repositories::postgres::platform_identity::PlatformIdentityRepository;
use maowbot_core::platforms::manager::PlatformManager;
use maowbot_core::services::user_service::UserService;
use maowbot_core::services::{CommandService, RedeemService};
use maowbot_common::traits::repository_traits::{CommandUsageRepository, RedeemUsageRepository, CommandRepository, CredentialsRepository, BotConfigRepository};
use maowbot_core::eventbus::EventBus;
use mockall::mock;
use async_trait::async_trait;

// Create mock implementations for all required dependencies
mock! {
    CommandUsageRepo {}
    #[async_trait]
    impl CommandUsageRepository for CommandUsageRepo {
        async fn insert_usage(&self, usage: &maowbot_common::models::command::CommandUsage) -> Result<(), Error>;
        async fn list_usage_for_command(&self, command_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::command::CommandUsage>, Error>;
        async fn list_usage_for_user(&self, user_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::command::CommandUsage>, Error>;
    }
}

mock! {
    RedeemUsageRepo {}
    #[async_trait]
    impl RedeemUsageRepository for RedeemUsageRepo {
        async fn insert_usage(&self, usage: &maowbot_common::models::redeem::RedeemUsage) -> Result<(), Error>;
        async fn list_usage_for_redeem(&self, redeem_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::redeem::RedeemUsage>, Error>;
        async fn list_usage_for_user(&self, user_id: uuid::Uuid, limit: i64) -> Result<Vec<maowbot_common::models::redeem::RedeemUsage>, Error>;
    }
}

mock! {
    CommandRepo {}
    #[async_trait]
    impl CommandRepository for CommandRepo {
        async fn create_command(&self, cmd: &maowbot_common::models::command::Command) -> Result<(), Error>;
        async fn list_commands(&self, platform: &str) -> Result<Vec<maowbot_common::models::command::Command>, Error>;
        async fn update_command(&self, cmd: &maowbot_common::models::command::Command) -> Result<(), Error>;
        async fn get_command_by_id(&self, command_id: uuid::Uuid) -> Result<Option<maowbot_common::models::command::Command>, Error>;
        async fn get_command_by_name(&self, platform: &str, command_name: &str) -> Result<Option<maowbot_common::models::command::Command>, Error>;
        async fn delete_command(&self, command_id: uuid::Uuid) -> Result<(), Error>;
    }
}

mock! {
    CredentialsRepo {}
    #[async_trait]
    impl CredentialsRepository for CredentialsRepo {
        async fn get_credentials(&self, platform: &maowbot_common::models::platform::Platform, user_id: uuid::Uuid) -> Result<Option<maowbot_common::models::platform::PlatformCredential>, Error>;
        async fn get_credential_by_id(&self, credential_id: uuid::Uuid) -> Result<Option<maowbot_common::models::platform::PlatformCredential>, Error>;
        async fn get_credential_by_provider_id(&self, platform: &maowbot_common::models::platform::Platform, provider_user_id: &str) -> Result<Option<maowbot_common::models::platform::PlatformCredential>, Error>;
        async fn save_credential(&self, cred: &maowbot_common::models::platform::PlatformCredential) -> Result<(), Error>;
        async fn list_credentials_for_platform(&self, platform: &maowbot_common::models::platform::Platform) -> Result<Vec<maowbot_common::models::platform::PlatformCredential>, Error>;
        async fn list_credentials_for_user(&self, user_id: uuid::Uuid) -> Result<Vec<maowbot_common::models::platform::PlatformCredential>, Error>;
        async fn delete_credential(&self, credential_id: uuid::Uuid) -> Result<(), Error>;
        async fn get_broadcaster_credential(&self, platform: &maowbot_common::models::platform::Platform) -> Result<Option<maowbot_common::models::platform::PlatformCredential>, Error>;
    }
}

mock! {
    BotConfigRepo {}
    #[async_trait]
    impl BotConfigRepository for BotConfigRepo {
        async fn get_config(&self, key: &str) -> Result<Option<serde_json::Value>, Error>;
        async fn set_config(&self, key: &str, value: &serde_json::Value) -> Result<(), Error>;
        async fn delete_config(&self, key: &str) -> Result<(), Error>;
        async fn list_keys(&self) -> Result<Vec<String>, Error>;
    }
}

mock! {
    RedeemRepo {}
    #[async_trait]
    impl maowbot_common::traits::repository_traits::RedeemRepository for RedeemRepo {
        async fn create_redeem(&self, redeem: &maowbot_common::models::redeem::Redeem) -> Result<(), Error>;
        async fn list_redeems(&self, platform: &str) -> Result<Vec<maowbot_common::models::redeem::Redeem>, Error>;
        async fn update_redeem(&self, redeem: &maowbot_common::models::redeem::Redeem) -> Result<(), Error>;
        async fn get_redeem_by_id(&self, redeem_id: uuid::Uuid) -> Result<Option<maowbot_common::models::redeem::Redeem>, Error>;
        async fn get_redeem_by_name(&self, platform: &str, redeem_name: &str) -> Result<Option<maowbot_common::models::redeem::Redeem>, Error>;
        async fn delete_redeem(&self, redeem_id: uuid::Uuid) -> Result<(), Error>;
    }
}

// This is a simplified test that checks that PluginManager can be created
#[tokio::test]
async fn test_plugin_manager_creation() -> Result<(), Error> {
    // Create mock objects for all dependencies
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://postgres:postgres@localhost/postgres")
        .await
        .unwrap_or_else(|_| panic!("Failed to create pool - this test requires a working postgres connection"));
    
    let user_repo = Arc::new(UserRepository::new(pool.clone()));
    let drip_repo = Arc::new(DripRepository::new(pool.clone()));
    let discord_repo = Arc::new(PostgresDiscordRepository::new(pool.clone()));
    let analytics_repo = Arc::new(PostgresAnalyticsRepository::new(pool.clone()));
    let user_analysis_repo = Arc::new(PostgresUserAnalysisRepository::new(pool.clone()));
    let platform_identity_repo = Arc::new(PlatformIdentityRepository::new(pool.clone()));
    let platform_manager = Arc::new(PlatformManager::new(
        Arc::new(UserService::new(Arc::new(UserRepository::new(pool.clone())))),
        Arc::new(crate::eventbus::EventBus::new()),
        Arc::new(MockCredentialsRepo::new()),
        Arc::new(PostgresDiscordRepository::new(pool.clone())),
    ));
    let user_service = Arc::new(UserService::new(Arc::new(UserRepository::new(pool.clone()))));
    
    // Create a mock CommandService since this test doesn't need a real one
    let command_repo = Arc::new(MockCommandRepo::new());
    let command_usage_repo = Arc::new(MockCommandUsageRepo::new());
    let credentials_repo = Arc::new(MockCredentialsRepo::new());
    let bot_config_repo = Arc::new(MockBotConfigRepo::new());
    
    let command_service = Arc::new(CommandService::new(
        command_repo,
        command_usage_repo,
        credentials_repo,
        user_service.clone(),
        bot_config_repo,
        platform_manager.clone(),
    ));
    // Create a mock RedeemService
    let redeem_repo = Arc::new(MockRedeemRepo::new());
    let redeem_usage_repo = Arc::new(MockRedeemUsageRepo::new());
    let redeem_service = Arc::new(RedeemService::new(
        redeem_repo, 
        redeem_usage_repo.clone(),
        user_service.clone(),
        platform_manager.clone(),
        credentials_repo.clone(),
    ));
    
    let mut mock_cmd_usage_repo = MockCommandUsageRepo::new();
    let mut mock_redeem_usage_repo = MockRedeemUsageRepo::new();

    // Set up any required mock behaviors
    mock_cmd_usage_repo.expect_insert_usage()
        .returning(|_| Ok(()));
    mock_cmd_usage_repo.expect_list_usage_for_command()
        .returning(|_, _| Ok(vec![]));
    mock_cmd_usage_repo.expect_list_usage_for_user()
        .returning(|_, _| Ok(vec![]));
        
    mock_redeem_usage_repo.expect_insert_usage()
        .returning(|_| Ok(()));
    mock_redeem_usage_repo.expect_list_usage_for_redeem()
        .returning(|_, _| Ok(vec![]));
    mock_redeem_usage_repo.expect_list_usage_for_user()
        .returning(|_, _| Ok(vec![]));

    let cmd_usage_repo: Arc<dyn CommandUsageRepository + Send + Sync> = Arc::new(mock_cmd_usage_repo);
    let redeem_usage_repo: Arc<dyn RedeemUsageRepository + Send + Sync> = Arc::new(mock_redeem_usage_repo);

    // Create the PluginManager with all required dependencies
    let passphrase = Some("test_passphrase".to_string());
    let pm = PluginManager::new(
        passphrase,
        user_repo,
        drip_repo,
        discord_repo,
        analytics_repo,
        user_analysis_repo,
        platform_identity_repo,
        platform_manager,
        user_service,
        command_service,
        redeem_service,
        cmd_usage_repo,
        redeem_usage_repo,
    );

    // Simple assertions to check the PluginManager was created successfully
    assert!(pm.passphrase.is_some());
    assert_eq!(pm.passphrase.unwrap(), "test_passphrase");
    assert!(pm.event_bus.is_none()); // Should be None initially
    
    Ok(())
}