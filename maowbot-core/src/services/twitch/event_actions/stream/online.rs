// File: maowbot-core/src/services/twitch/event_actions/stream/offline.rs

use crate::Error;
use crate::platforms::twitch_eventsub::events::{StreamOffline, StreamOnline};
use crate::services::user_service::UserService;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::repositories::BotConfigRepository;
use crate::tasks::redeem_sync;

pub async fn handle_stream_online(
    evt: StreamOnline,
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
) -> Result<(), Error> {
    // 1) Optionally do your custom logic with 'evt'
    // e.g. store something in DB, log something, etc.

    // 2) Then call the redeem sync if you want
    redeem_sync::sync_channel_redeems(
        redeem_service,
        platform_manager,
        user_service,
        bot_config_repo,
        false
    ).await?;

    Ok(())
}
