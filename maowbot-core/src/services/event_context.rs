use std::sync::Arc;
use crate::platforms::manager::PlatformManager;
use crate::services::user_service::UserService;
use crate::services::RedeemService;
use crate::services::message_service::MessageService;
use crate::services::MessageSender;
use crate::services::osc_toggle_service::OscToggleService;
use crate::repositories::postgres::discord::PostgresDiscordRepository;
use maowbot_common::traits::repository_traits::{BotConfigRepository, CredentialsRepository};

/// EventContext encapsulates all services that event handlers might need.
/// This allows us to pass a single object to handlers instead of many parameters,
/// making it easier to extend and test.
#[derive(Clone)]
pub struct EventContext {
    pub platform_manager: Arc<PlatformManager>,
    pub user_service: Arc<UserService>,
    pub redeem_service: Arc<RedeemService>,
    pub message_service: Arc<MessageService>,
    pub message_sender: Arc<MessageSender>,
    pub osc_toggle_service: Arc<OscToggleService>,
    pub bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
    pub discord_repo: Arc<PostgresDiscordRepository>,
    pub credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
}

impl EventContext {
    pub fn new(
        platform_manager: Arc<PlatformManager>,
        user_service: Arc<UserService>,
        redeem_service: Arc<RedeemService>,
        message_service: Arc<MessageService>,
        message_sender: Arc<MessageSender>,
        osc_toggle_service: Arc<OscToggleService>,
        bot_config_repo: Arc<dyn BotConfigRepository + Send + Sync>,
        discord_repo: Arc<PostgresDiscordRepository>,
        credentials_repo: Arc<dyn CredentialsRepository + Send + Sync>,
    ) -> Self {
        Self {
            platform_manager,
            user_service,
            redeem_service,
            message_service,
            message_sender,
            osc_toggle_service,
            bot_config_repo,
            discord_repo,
            credentials_repo,
        }
    }
}