use tracing::{info, warn, error};
use serde_json::json;
use uuid::Uuid;
use maowbot_common::models::user::User;
use maowbot_ai::plugins::ai_service::AiService;
use maowbot_common::traits::api::AiApi;
use std::sync::Arc;
use crate::Error;
use crate::services::twitch::redeem_service::RedeemHandlerContext;
use crate::platforms::twitch::requests::channel_points::Redemption;
use crate::services::message_sender::{MessageSender, MessageResponse};

// Helper function to generate an AI text response
async fn generate_ai_response(
    ctx: &RedeemHandlerContext<'_>,
    user_id: Uuid,
    input: &str,
    system_prompt: Option<&str>
) -> Result<String, Error> {
    info!("Generating AI response for user {}", user_id);
    
    // Get the AI API through the redeem service first
    let ai_api_opt = match ctx.redeem_service.get_ai_api() {
        Some(api) => Some(api),
        None => ctx.redeem_service.platform_manager.get_ai_api()
    };
    
    // If still not found, try through plugin manager
    let ai_api = if let Some(api) = ai_api_opt {
        api
    } else if let Some(plugin_manager) = ctx.redeem_service.platform_manager.plugin_manager() {
        if let Some(ai_impl) = &plugin_manager.ai_api_impl {
            Arc::new(ai_impl.clone())
        } else {
            warn!("AI API is not available through any means, falling back to placeholder response");
            if let Some(prompt) = system_prompt {
                return Ok(format!("AI response to '{}' with prompt '{}'", input, prompt));
            } else {
                return Ok(format!("AI response to '{}'", input));
            }
        }
    } else {
        warn!("AI API is not available through any means, falling back to placeholder response");
        if let Some(prompt) = system_prompt {
            return Ok(format!("AI response to '{}' with prompt '{}'", input, prompt));
        } else {
            return Ok(format!("AI response to '{}'", input));
        }
    };
    
    // If we have a system prompt, construct a message array with it
    if let Some(prompt) = system_prompt {
        info!("Using system prompt: {}", prompt);
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": prompt
            }),
            serde_json::json!({
                "role": "user",
                "content": input
            })
        ];
        
        // Use the generic chat endpoint without function calling
        match ai_api.generate_chat(messages).await {
            Ok(response) => Ok(response),
            Err(e) => {
                error!("Error generating AI response: {:?}", e);
                Err(Error::Internal(format!("AI API error: {}", e)))
            }
        }
    } else {
        // Create a basic message with just the user input
        let messages = vec![
            serde_json::json!({
                "role": "user",
                "content": input
            })
        ];
        
        // Use the generic chat endpoint without function calling
        match ai_api.generate_chat(messages).await {
            Ok(response) => Ok(response),
            Err(e) => {
                error!("Error generating AI response: {:?}", e);
                Err(Error::Internal(format!("AI API error: {}", e)))
            }
        }
    }
}

// Helper function to generate an AI response with web search capability
async fn generate_ai_web_search_response(
    ctx: &RedeemHandlerContext<'_>,
    user_id: Uuid,
    input: &str,
    system_prompt: Option<&str>
) -> Result<String, Error> {
    info!("Generating AI web search response for user {}", user_id);
    
    // Get the AI API the same way as in generate_ai_response
    let ai_api_opt = match ctx.redeem_service.get_ai_api() {
        Some(api) => Some(api),
        None => ctx.redeem_service.platform_manager.get_ai_api()
    };
    
    let ai_api = if let Some(api) = ai_api_opt {
        api
    } else if let Some(plugin_manager) = ctx.redeem_service.platform_manager.plugin_manager() {
        if let Some(ai_impl) = &plugin_manager.ai_api_impl {
            Arc::new(ai_impl.clone())
        } else {
            warn!("AI API is not available through any means, falling back to placeholder response");
            if let Some(prompt) = system_prompt {
                return Ok(format!("Web search AI response to '{}' with prompt '{}'", input, prompt));
            } else {
                return Ok(format!("Web search AI response to '{}'", input));
            }
        }
    } else {
        warn!("AI API is not available through any means, falling back to placeholder response");
        if let Some(prompt) = system_prompt {
            return Ok(format!("Web search AI response to '{}' with prompt '{}'", input, prompt));
        } else {
            return Ok(format!("Web search AI response to '{}'", input));
        }
    };
    
    // Create messages with system prompt if provided
    let mut messages = Vec::new();
    
    if let Some(prompt) = system_prompt {
        messages.push(serde_json::json!({
            "role": "system",
            "content": prompt
        }));
    }
    
    messages.push(serde_json::json!({
        "role": "user",
        "content": input
    }));
    
    // The OpenAI provider has been modified to automatically add web search options
    // when the model is set to gpt-4.1 or when "enable_web_search" is set to "true"
    // in the options. Here we'll use both approaches to ensure it works.
    
    // Configure the AI service to use gpt-4o-search-preview model with web search
    if let Err(e) = ai_api.configure_ai_provider(serde_json::json!({
        "provider_type": "openai",
        "default_model": "gpt-4o-search-preview",
        "options": {
            "enable_web_search": "true"
        }
    })).await {
        warn!("Failed to configure AI provider for web search: {:?}", e);
        // We'll try a simpler approach, directly setting the system message to indicate web search
        info!("Falling back to simpler web search approach");
    }
    
    // Double-check that web search is enabled by adding a search hint to the input
    let enhanced_input = if !input.contains("[search]") {
        format!("[search] {}", input)
    } else {
        input.to_string()
    };
    
    info!("Using model gpt-4o-search-preview with web search capabilities");
    
    // Update the user message to use the enhanced input with search hint
    messages.pop(); // Remove the original user message
    messages.push(serde_json::json!({
        "role": "user",
        "content": enhanced_input
    }));
    
    // Process the request - the provider will add web_search_options
    match ai_api.generate_chat(messages).await {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Error generating AI web search response: {:?}", e);
            Err(Error::Internal(format!("AI API web search error: {}", e)))
        }
    }
}

// Helper function to send AI response to chat using the shared MessageSender
async fn send_ai_response_to_chat(
    ctx: &RedeemHandlerContext<'_>,
    channel: &str,
    response: &str,
) -> Result<(), Error> {
    info!("Sending AI response to chat channel: {}", channel);
    
    // Create a message sender instance using the platform manager and credentials repo
    let message_sender = MessageSender::new(
        ctx.redeem_service.credentials_repo.clone(),
        ctx.redeem_service.platform_manager.clone()
    );
    
    // Get the credential ID from the active credential if available
    let specified_credential_id = ctx.active_credential.as_ref().map(|cred| cred.credential_id);
    
    // Send the message using our shared message sender
    let result = message_sender.send_twitch_message(
        channel,
        response,
        specified_credential_id,
        // Use Uuid::nil() as a fallback if no user is available
        Uuid::nil()
    ).await;
    
    match result {
        Ok(_) => {
            info!("Successfully sent AI response to channel: {}", channel);
            Ok(())
        },
        Err(e) => {
            error!("Failed to send AI response: {:?}", e);
            Err(e)
        }
    }
}

// Helper function to convert Twitch user ID string to UUID user ID
async fn get_user_from_twitch_id(
    ctx: &RedeemHandlerContext<'_>, 
    twitch_user_id: &str
) -> Result<User, Error> {
    // Use "twitch-irc" as the platform for consistent user lookup
    match ctx.redeem_service.user_service.get_or_create_user(
        "twitch-irc",
        twitch_user_id,
        Some(twitch_user_id)
    ).await {
        Ok(user) => Ok(user),
        Err(e) => Err(Error::Internal(format!("Error getting user: {}", e)))
    }
}

/// Handles the standard AI redeem that performs a serious AI response
pub async fn handle_askai_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
) -> Result<(), Error> {
    info!(
        "Builtin 'ask ai' redeem triggered for user_id={} reward='{}'",
        redemption.user_id, redemption.reward.title
    );

    // Get the user input from the redemption
    let user_input = if !redemption.user_input.trim().is_empty() {
        redemption.user_input.trim()
    } else {
        // No user input or empty input, mark as failed
        let helix_client_opt = ctx.redeem_service.platform_manager.get_twitch_client().await;
        
        // Try to use the Helix client from either source to cancel the redemption
        info!("No user input provided for 'ask ai' redeem, canceling redemption");
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;

        // First try with context client
        let update_result = if let Some(client) = &ctx.helix_client {
            client.update_redemption_status(
                broadcaster_id, 
                reward_id, 
                &[&redemption_id],
                "CANCELED"
            ).await
        } else if let Some(client) = helix_client_opt {
            // Fall back to platform manager client
            client.update_redemption_status(
                broadcaster_id, 
                reward_id, 
                &[&redemption_id],
                "CANCELED"
            ).await
        } else {
            // No client available
            warn!("No Helix client available, but continuing anyway with empty response");
            return Ok(());
        };
        
        // Process the result
        match update_result {
            Ok(_) => {
                info!("Successfully canceled empty redeem");
                return Ok(());
            },
            Err(e) => {
                warn!("Failed to cancel empty redeem: {:?}", e);
                return Err(Error::Internal(format!("Failed to cancel redeem: {}", e)));
            }
        }
    };

    info!("Received askai redeem with input: {}", user_input);
    
    // Get the user from the Twitch ID
    let user = match get_user_from_twitch_id(ctx, &redemption.user_id).await {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to get user for AI redeem: {:?}", e);
            
            // Try to cancel the redemption since we can't process it
            let helix_client_opt = ctx.redeem_service.platform_manager.get_twitch_client().await;
            
            if let Some(client) = &ctx.helix_client {
                let _ = client
                    .update_redemption_status(
                        &redemption.broadcaster_id,
                        &redemption.reward.id,
                        &[&redemption.id],
                        "CANCELED",
                    )
                    .await;
            } else if let Some(client) = helix_client_opt {
                let _ = client
                    .update_redemption_status(
                        &redemption.broadcaster_id,
                        &redemption.reward.id,
                        &[&redemption.id],
                        "CANCELED",
                    )
                    .await;
            }
            
            return Err(e);
        }
    };
    
    // Configure the AI API to use gpt-4o without web search
    let ai_api_opt = match ctx.redeem_service.get_ai_api() {
        Some(api) => Some(api),
        None => ctx.redeem_service.platform_manager.get_ai_api()
    };
    
    if let Some(ai_api) = ai_api_opt {
        // Configure the provider explicitly to use gpt-4o without web search
        if let Err(e) = ai_api.configure_ai_provider(serde_json::json!({
            "provider_type": "openai",
            "default_model": "gpt-4o",
            "options": {
                "enable_web_search": "false"
            }
        })).await {
            warn!("Failed to configure AI provider for standard response: {:?}", e);
        }
    }
    
    // Generate an AI response using real AI API
    let response = match generate_ai_response(ctx, user.user_id, user_input, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Error generating AI response: {:?}", e);
            format!("Sorry, I couldn't generate a response: {}", e)
        }
    };
    
    // Send the response to chat
    if let Some(broadcaster_login) = &redemption.broadcaster_login {
        if let Err(e) = send_ai_response_to_chat(ctx, broadcaster_login, &response).await {
            error!("Failed to send AI response to chat: {:?}", e);
        }
    } else {
        error!("No broadcaster login found in redemption - can't send response");
    }
    
    // Try to mark the redemption as complete
    let helix_client_opt = ctx.redeem_service.platform_manager.get_twitch_client().await;
    let broadcaster_id = &redemption.broadcaster_id;
    let reward_id = &redemption.reward.id;
    let redemption_id = &redemption.id;
    
    info!("Attempting to complete AI redeem");
    
    if let Some(client) = &ctx.helix_client {
        if let Err(e) = client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[&redemption_id],
                "FULFILLED",
            )
            .await
        {
            warn!("Failed to mark redemption as fulfilled using context client: {:?}", e);
            
            // Try fall back to platform manager client if context client fails
            if let Some(client2) = helix_client_opt {
                let _ = client2
                    .update_redemption_status(
                        broadcaster_id,
                        reward_id,
                        &[&redemption_id],
                        "FULFILLED",
                    )
                    .await;
            }
        }
    } else if let Some(client) = helix_client_opt {
        // Fall back to platform manager client
        let _ = client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[&redemption_id],
                "FULFILLED",
            )
            .await;
    } else {
        warn!("No Helix client available from any source, can't update redeem status");
    }

    Ok(())
}

/// Handles the "ask maow" redeem that provides a humorous AI response
pub async fn handle_askmao_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
) -> Result<(), Error> {
    info!(
        "Builtin 'ask maow' redeem triggered for user_id={} reward='{}'",
        redemption.user_id, redemption.reward.title
    );

    // Get the user input from the redemption
    let user_input = if !redemption.user_input.trim().is_empty() {
        redemption.user_input.trim()
    } else {
        // No user input or empty input, mark as failed
        if let Some(client) = &ctx.helix_client {
            let broadcaster_id = &redemption.broadcaster_id;
            let reward_id = &redemption.reward.id;
            let redemption_id = &redemption.id;

            info!("No user input provided for 'ask maow' redeem, canceling redemption");

            // Cancel by setting status = "CANCELED"
            let _ = client
                .update_redemption_status(
                    broadcaster_id,
                    reward_id,
                    &[&redemption_id],
                    "CANCELED",
                )
                .await?;
            return Ok(());
        } else {
            return Err(Error::Internal("No Helix client available".to_string()));
        }
    };

    info!("Received askmao redeem with input: {}", user_input);
    
    // Get the user from the Twitch ID
    let user = match get_user_from_twitch_id(ctx, &redemption.user_id).await {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to get user for askmaow redeem: {:?}", e);
            
            // Try to cancel the redemption since we can't process it
            if let Some(client) = &ctx.helix_client {
                let _ = client
                    .update_redemption_status(
                        &redemption.broadcaster_id,
                        &redemption.reward.id,
                        &[&redemption.id],
                        "CANCELED",
                    )
                    .await;
            }
            
            return Err(e);
        }
    };
    
    // Configure the AI API to use gpt-4o without web search
    let ai_api_opt = match ctx.redeem_service.get_ai_api() {
        Some(api) => Some(api),
        None => ctx.redeem_service.platform_manager.get_ai_api()
    };
    
    if let Some(ai_api) = ai_api_opt {
        // Configure the provider explicitly to use gpt-4o without web search
        if let Err(e) = ai_api.configure_ai_provider(serde_json::json!({
            "provider_type": "openai",
            "default_model": "gpt-4o",
            "options": {
                "enable_web_search": "false"
            }
        })).await {
            warn!("Failed to configure AI provider for cat-like response: {:?}", e);
        }
    }
    
    // Create a cat-like prompt for Maowbot
    let system_prompt = "You are Maowbot, a sassy and humorous cat-like AI. Respond with cat-like mannerisms, occasional 'meow' sounds, and a playful attitude. Your responses should be brief, funny, and slightly sarcastic while still being helpful. Limit responses to 1-2 sentences when possible.";
    
    // Generate an AI response with cat-like system prompt using real AI API
    let response = match generate_ai_response(ctx, user.user_id, user_input, Some(system_prompt)).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Error generating cat-like AI response: {:?}", e);
            format!("Meow? *looks confused* Something went wrong with my cat brain. Try again later!")
        }
    };
    
    // Send the response to chat
    if let Some(broadcaster_login) = &redemption.broadcaster_login {
        if let Err(e) = send_ai_response_to_chat(ctx, broadcaster_login, &response).await {
            error!("Failed to send askmaow response to chat: {:?}", e);
        }
    } else {
        error!("No broadcaster login found in redemption");
    }
    
    // Mark the redemption as complete
    if let Some(client) = &ctx.helix_client {
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;

        info!("Completing askmaow redeem");
        
        // Set status to "FULFILLED"
        let _ = client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[&redemption_id],
                "FULFILLED",
            )
            .await?;
    } else {
        warn!("No Helix client available, can't update redeem status");
    }

    Ok(())
}

/// Handles the "ask ai with search" redeem that performs an AI response with web search
pub async fn handle_askai_search_redemption(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
) -> Result<(), Error> {
    info!(
        "Builtin 'ask ai with search' redeem triggered for user_id={} reward='{}'",
        redemption.user_id, redemption.reward.title
    );

    // Get the user input from the redemption
    let user_input = if !redemption.user_input.trim().is_empty() {
        redemption.user_input.trim()
    } else {
        // No user input or empty input, mark as failed
        if let Some(client) = &ctx.helix_client {
            let broadcaster_id = &redemption.broadcaster_id;
            let reward_id = &redemption.reward.id;
            let redemption_id = &redemption.id;

            info!("No user input provided for 'ask ai with search' redeem, canceling redemption");

            // Cancel by setting status = "CANCELED"
            let _ = client
                .update_redemption_status(
                    broadcaster_id,
                    reward_id,
                    &[&redemption_id],
                    "CANCELED",
                )
                .await?;
            return Ok(());
        } else {
            return Err(Error::Internal("No Helix client available".to_string()));
        }
    };

    info!("Received askai_search redeem with input: {}", user_input);
    
    // Get the user from the Twitch ID
    let user = match get_user_from_twitch_id(ctx, &redemption.user_id).await {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to get user for AI search redeem: {:?}", e);
            
            // Try to cancel the redemption since we can't process it
            if let Some(client) = &ctx.helix_client {
                let _ = client
                    .update_redemption_status(
                        &redemption.broadcaster_id,
                        &redemption.reward.id,
                        &[&redemption.id],
                        "CANCELED",
                    )
                    .await;
            }
            
            return Err(e);
        }
    };
    
    // Create a search prompt for web-capable AI that works well for Twitch chat
    let system_prompt = "You are a helpful AI assistant with the ability to search the web for the most up-to-date information. Your responses will be shown in Twitch chat, so they MUST be brief (1-3 sentences max) while still being informative. Begin your response with 'Search result:' and include at least one source URL in [square brackets] at the end where appropriate. Use casual, conversational language suitable for a Twitch audience.";
    
    // Generate an AI response using the web search capability
    info!("Attempting web search AI response generation with enhanced error handling");
    let response = match generate_ai_web_search_response(ctx, user.user_id, user_input, Some(system_prompt)).await {
        Ok(resp) => {
            info!("Successfully generated web search response: {}", resp);
            resp
        },
        Err(e) => {
            error!("Error generating search AI response: {:?}", e);
            
            // Try a fallback approach - use standard AI response but prefix with search results
            info!("Attempting fallback to standard AI with search prefix");
            match generate_ai_response(ctx, user.user_id, 
                &format!("Search the web for the following query, then provide a brief answer with sources: {}", user_input), 
                Some("You are a helpful search assistant. Provide brief answers with source URLs in [brackets].")
            ).await {
                Ok(fallback_resp) => {
                    info!("Fallback successful");
                    fallback_resp
                },
                Err(fallback_err) => {
                    error!("Fallback also failed: {:?}", fallback_err);
                    format!("Search Results: I couldn't perform a search due to a technical error. Please try again later.")
                }
            }
        }
    };
    
    // Send the response to chat
    if let Some(broadcaster_login) = &redemption.broadcaster_login {
        if let Err(e) = send_ai_response_to_chat(ctx, broadcaster_login, &response).await {
            error!("Failed to send AI search response to chat: {:?}", e);
        }
    } else {
        error!("No broadcaster login found in redemption");
    }
    
    // Mark the redemption as complete
    if let Some(client) = &ctx.helix_client {
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;

        info!("Completing AI search redeem");
        
        // Set status to "FULFILLED"
        let _ = client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[&redemption_id],
                "FULFILLED",
            )
            .await?;
    } else {
        warn!("No Helix client available, can't update redeem status");
    }

    Ok(())
}