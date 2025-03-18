// ========================================================
// File: maowbot-core/src/services/twitch/event_actions/stream/online.rs
// ========================================================
use maowbot_common::traits::repository_traits::BotConfigRepository;
use maowbot_common::models::platform::Platform;
use crate::Error;
use crate::platforms::twitch_eventsub::events::StreamOnline;
use crate::services::user_service::UserService;
use crate::services::twitch::redeem_service::RedeemService;
use crate::platforms::manager::PlatformManager;
use crate::repositories::postgres::discord::PostgresDiscordRepository;
use crate::tasks::redeem_sync;
use tracing::{debug, info};

// Import the updated fetch_stream_details function and TwitchHelixClient.
use crate::platforms::twitch::client::TwitchHelixClient;
use crate::platforms::twitch::requests::stream::fetch_stream_details;

/// Handles the Twitch stream.online event by fetching real-time stream details.
pub async fn handle_stream_online(
    evt: StreamOnline,
    redeem_service: &RedeemService,
    platform_manager: &PlatformManager,
    user_service: &UserService,
    bot_config_repo: &dyn BotConfigRepository,
    discord_repo: &PostgresDiscordRepository,
) -> Result<(), Error> {
    debug!("Entered handle_stream_online with event: {:?}", evt);

    // 1) Retrieve the broadcaster credential for Twitch.
    let broadcaster_cred_opt = platform_manager
        .credentials_repo
        .get_broadcaster_credential(&Platform::Twitch)
        .await?;

    if let Some(broadcaster_cred) = broadcaster_cred_opt {
        // Use the broadcaster's username from the credential.
        let broadcaster_name = broadcaster_cred.user_name.clone();
        let link = format!("https://twitch.tv/{}", broadcaster_name);

        // 2) Fetch additional stream details from Twitch using real-time API data.
        let twitch_client = platform_manager
            .get_twitch_client()
            .await
            .ok_or_else(|| Error::Platform("Twitch client not available".into()))?;
        let details = fetch_stream_details(&twitch_client, &broadcaster_name).await?;

        // 3) Look up the Discord event config for "stream.online".
        if let Some(config) = discord_repo.get_event_config_by_name("stream.online").await? {
            // Determine which account to send from.
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

            // If any ping roles are set, format them as Discord role mentions.
            let ping_str = if let Some(roles) = &config.ping_roles {
                if !roles.is_empty() {
                    roles.iter()
                        .map(|r| format!("<@&{}>", r))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            };

            // Build the go-live message with updated Twitch details.
            let go_live_msg = format!(
                "🔴 **{}** is now live!\nTitle: {}\nPlaying: {}\n[Game Thumbnail]({})\nProfile: {}\nJoin at: {}\n{}",
                details.broadcaster_name,
                details.stream_title,
                details.game,
                details.game_thumbnail,
                details.pfp,
                link,
                ping_str
            );
            info!("Sending Discord message from account '{}': {}", account_name, go_live_msg);

            // 4) Send the Discord message.
            platform_manager
                .send_discord_message(
                    &account_name,
                    &config.guild_id,
                    &config.channel_id,
                    &go_live_msg,
                )
                .await?;
        }
    }

    // 5) Optionally, perform redeem sync.
    redeem_sync::sync_channel_redeems(
        redeem_service,
        platform_manager,
        user_service,
        bot_config_repo,
        false,
    )
        .await?;

    Ok(())
}
