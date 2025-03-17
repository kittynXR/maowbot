// File: maowbot-core/src/services/twitch/event_actions/stream/online.rs

use maowbot_common::traits::repository_traits::BotConfigRepository;
use maowbot_common::models::platform::Platform;
use crate::Error;
use crate::platforms::twitch_eventsub::events::StreamOnline;
use crate::services::user_service::UserService;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::repositories::postgres::discord::PostgresDiscordRepository;
use crate::tasks::redeem_sync;


pub async fn handle_stream_online(
    evt: StreamOnline,
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
    discord_repo: &PostgresDiscordRepository,
) -> Result<(), Error> {
    // 1) Possibly do your custom logic with 'evt'
    // e.g. store something, log, etc.
    // You have event data in `evt`, e.g. `evt.broadcaster_user_id` and so on.

    // 2) Retrieve the broadcaster credential for Twitch
    let broadcaster_cred_opt = platform_manager
        .credentials_repo
        .get_broadcaster_credential(&Platform::Twitch)
        .await?;

    // 3) If found, create a "We are live" message
    if let Some(broadcaster_cred) = broadcaster_cred_opt {
        let twitch_name = &broadcaster_cred.user_name;
        let link = format!("https://twitch.tv/{}", twitch_name);
        let go_live_msg = format!("🔴 The stream is now live! Join at: {link}");

        // 4) Look up the config row for event_name = "stream.online"
        if let Some(config) = discord_repo.get_event_config_by_name("stream.online").await? {
            // If respond_with_credential is set, use that. Otherwise use some default:
            let account_name = if let Some(cred_id) = config.respond_with_credential {
                // fetch that credential to see the .user_name
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

            // 5) Send Discord message
            //    We call platform_manager.send_discord_message(account_name, guild_id, channel_id, text).
            platform_manager
                .send_discord_message(
                    &account_name,
                    &config.guild_id,
                    &config.channel_id,
                    &go_live_msg
                )
                .await?;
        }
    }

    // 6) Then do the redeem sync if you want
    redeem_sync::sync_channel_redeems(
        redeem_service,
        platform_manager,
        user_service,
        bot_config_repo,
        false
    ).await?;

    Ok(())
}
