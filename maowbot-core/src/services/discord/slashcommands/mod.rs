// File: maowbot-core/src/services/discord/slashcommands/mod.rs

pub mod ping;

use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::{
    application::{
        command::Command,
        interaction::{Interaction, InteractionData},
    },
    gateway::payload::incoming::InteractionCreate,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    id::marker::{ApplicationMarker, InteractionMarker},
    id::Id,
};
use twilight_util::builder::command::CommandBuilder;

use maowbot_common::error::Error;
use crate::services::discord::slashcommands::ping::{
    create_ping_command,
    handle_ping_interaction,
};


pub async fn register_global_slash_commands(
    http: &Arc<HttpClient>,
    application_id: Id<ApplicationMarker>,
) -> Result<(), Error> {
    // Build your slash commands:
    let ping_cmd = create_ping_command().build(); // returns `Command` immediately
    let commands = &[ping_cmd]; // If more commands, push them here.

    http.interaction(application_id)
        .set_global_commands(commands)
        .await
        .map_err(|e| Error::Platform(format!("Failed to register global slash commands: {e}")))?;

    Ok(())
}

/// Dispatch slash commands from an `InteractionCreate`.
pub async fn handle_interaction_create(
    http: Arc<HttpClient>,
    application_id: Id<ApplicationMarker>,
    event: &InteractionCreate,
) -> Result<(), Error> {
    let interaction = &event.0;
    let interaction_id = interaction.id;
    let interaction_token = &interaction.token;

    // Only handle ApplicationCommand interactions:
    if let Some(InteractionData::ApplicationCommand(cmd_data)) = &interaction.data {
        let name = cmd_data.name.as_str();
        match name {
            "ping" => {
                handle_ping_interaction(&http, application_id, interaction_id, interaction_token).await?;
            }
            other => {
                // For unknown commands, respond with error:
                http.interaction(application_id)
                    .create_response(
                        interaction_id,
                        interaction_token,
                        &InteractionResponse {
                            kind: InteractionResponseType::ChannelMessageWithSource,
                            data: Some(InteractionResponseData {
                                content: Some(format!("Unrecognized command: {other}")),
                                ..Default::default()
                            }),
                        },
                    )
                    .await
                    .ok(); // ignore error
            }
        }
    }

    Ok(())
}
