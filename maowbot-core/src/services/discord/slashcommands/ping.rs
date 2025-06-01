// File: maowbot-core/src/services/discord/slashcommands/ping.rs

use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::{
    application::interaction::Interaction,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    id::marker::{ApplicationMarker, InteractionMarker},
    id::Id,
};
use twilight_util::builder::command::CommandBuilder;

use maowbot_common::error::Error;

/// Create a CommandBuilder for `/ping`.
/// In Twilight 0.16, `build()` returns a `Command` directly (no `Result`).
pub fn create_ping_command() -> CommandBuilder {
    CommandBuilder::new(
        "ping",
        "Replies with 'pong!'",
        twilight_model::application::command::CommandType::ChatInput,
    )
        // (Optional) If you want slash cmd to be usable in DMs:
        .dm_permission(true)
}

/// Handle an incoming `/ping` interaction.
pub async fn handle_ping_interaction(
    http: &Arc<HttpClient>,
    application_id: Id<ApplicationMarker>,
    interaction_id: Id<InteractionMarker>,
    interaction_token: &str,
) -> Result<(), Error> {
    http.interaction(application_id)
        .create_response(
            interaction_id,
            interaction_token,
            &InteractionResponse {
                kind: InteractionResponseType::ChannelMessageWithSource,
                data: Some(InteractionResponseData {
                    content: Some("pong!".into()),
                    // If you want ephemeral: flags: Some(MessageFlags::EPHEMERAL.bits()),
                    ..Default::default()
                }),
            },
        )
        .await
        .map_err(|e| Error::Platform(format!("Error responding to `/ping`: {e}")))?;

    Ok(())
}
