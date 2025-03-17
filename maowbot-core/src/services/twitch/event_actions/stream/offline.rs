// File: maowbot-core/src/services/twitch/event_actions/stream/offline.rs

use maowbot_common::traits::repository_traits::BotConfigRepository;
use maowbot_common::models::platform::Platform;
use crate::Error;
use crate::platforms::twitch_eventsub::events::StreamOffline;
use crate::services::user_service::UserService;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::tasks::redeem_sync;

use crate::repositories::postgres::discord::PostgresDiscordRepository;

pub async fn handle_stream_offline(
    evt: StreamOffline,
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
    // As with online, we assume we can pass this in for easy usage:
    discord_repo: &PostgresDiscordRepository,
) -> Result<(), Error> {
    // 1) Possibly do your custom logic with 'evt'
    // e.g. store something in DB, or log a message
    let broadcaster_cred_opt = platform_manager
        .credentials_repo
        .get_broadcaster_credential(&Platform::Twitch)
        .await?;

    if let Some(broadcaster_cred) = broadcaster_cred_opt {
        let twitch_name = &broadcaster_cred.user_name;
        let offline_msg = format!("🛑 The stream just went offline. Thanks for watching {twitch_name}!");

        // 2) Look up the config row for event_name = "stream.offline"
        if let Some(config) = discord_repo.get_event_config_by_name("stream.offline").await? {
            let account_name = if let Some(cred_id) = config.respond_with_credential {
                if let Some(dc_cred) = platform_manager
                    .credentials_repo
                    .get_credential_by_id(cred_id)
                    .await?
                {
                    dc_cred.user_name
                } else {
                    "cutecat_chat".to_string()
                }
            } else {
                "cutecat_chat".to_string()
            };

            platform_manager
                .send_discord_message(
                    &account_name,
                    &config.guild_id,
                    &config.channel_id,
                    &offline_msg
                )
                .await?;
        }
    }

    // 3) Then call redeem sync if desired
    redeem_sync::sync_channel_redeems(
        redeem_service,
        platform_manager,
        user_service,
        bot_config_repo,
        false
    ).await?;

    Ok(())
}
