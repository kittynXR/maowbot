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

// Helper function to generate an AI text response
async fn generate_ai_response(
    ctx: &RedeemHandlerContext<'_>,
    user_id: Uuid,
    input: &str,
    system_prompt: Option<&str>
) -> Result<String, Error> {
    info!("🔍 ASKAI: Generating AI response for user {}, input: '{}'", user_id, input);
    // Get the AI API directly through multiple methods to increase chances of success
    
    // First try through the redeem service with more detailed logging
    let ai_api_opt = match ctx.redeem_service.get_ai_api() {
        Some(api) => {
            info!("🔍 ASKAI: Successfully got AI API from redeem_service.get_ai_api()");
            // Test if this API actually works
            match api.process_user_message(Uuid::nil(), "Test message for AI verification").await {
                Ok(test_response) => {
                    info!("🔍 ASKAI: Test call to AI API successful! Response: '{}'", test_response);
                    Some(api)
                },
                Err(e) => {
                    error!("🔍 ASKAI: AI API from redeem_service failed test: {:?}", e);
                    None
                }
            }
        },
        None => {
            info!("🔍 ASKAI: Failed to get AI API from redeem_service.get_ai_api(), trying through platform_manager");
            None
        }
    };
    
    // Either use what we got from redeem_service or try from platform_manager
    let ai_api = if let Some(api) = ai_api_opt {
        api
    } else if let Some(api) = ctx.redeem_service.platform_manager.get_ai_api() {
        info!("🔍 ASKAI: Successfully got AI API directly from platform_manager.get_ai_api()");
        // Test the platform manager API too
        match api.process_user_message(Uuid::nil(), "Test message for AI verification").await {
            Ok(test_response) => {
                info!("🔍 ASKAI: Test call to platform_manager's AI API successful! Response: '{}'", test_response);
            },
            Err(e) => {
                error!("🔍 ASKAI: AI API from platform_manager failed test: {:?}", e);
            }
        }
        api
    } else {
        // Last try - check if plugin_manager is available and has the AI API
        if let Some(plugin_manager) = ctx.redeem_service.platform_manager.plugin_manager() {
            if let Some(ai_impl) = &plugin_manager.ai_api_impl {
                info!("🔍 ASKAI: Using AI API implementation from plugin_manager");
                // Wrap and test one more time
                let api = Arc::new(ai_impl.clone());
                match api.process_user_message(Uuid::nil(), "Test message for AI verification").await {
                    Ok(test_response) => {
                        info!("🔍 ASKAI: Test call to plugin_manager's AI API successful! Response: '{}'", test_response);
                    },
                    Err(e) => {
                        error!("🔍 ASKAI: AI API from plugin_manager failed test: {:?}", e);
                    }
                }
                api
            } else {
                warn!("🔍 ASKAI: AI API is not available through any means, falling back to placeholder response");
                // Continue with placeholder response
                if let Some(prompt) = system_prompt {
                    return Ok(format!("AI response to '{}' with prompt '{}'", input, prompt));
                } else {
                    return Ok(format!("AI response to '{}'", input));
                }
            }
        } else {
            warn!("🔍 ASKAI: AI API is not available through any means, falling back to placeholder response");
            // Continue with placeholder response
            if let Some(prompt) = system_prompt {
                return Ok(format!("AI response to '{}' with prompt '{}'", input, prompt));
            } else {
                return Ok(format!("AI response to '{}'", input));
            }
        }
    };
    
    info!("🔍 ASKAI: Using AI API to generate response for input: {}", input);
    
    // If we have a system prompt, construct a message array with it
    if let Some(prompt) = system_prompt {
        info!("🔍 ASKAI: Using system prompt: {}", prompt);
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
        
        // Use the generate_chat method to get a response
        info!("🔍 ASKAI: Calling generate_chat with {} messages", messages.len());
        match ai_api.generate_chat(messages).await {
            Ok(response) => {
                info!("🔍 ASKAI: AI response generated successfully: '{}'", response);
                Ok(response)
            },
            Err(e) => {
                error!("🔍 ASKAI: Error generating AI response: {:?}", e);
                Err(Error::Internal(format!("AI API error: {}", e)))
            }
        }
    } else {
        // Use the process_user_message method that handles conversation history
        info!("🔍 ASKAI: No system prompt provided, using process_user_message with user_id {}", user_id);
        match ai_api.process_user_message(user_id, input).await {
            Ok(response) => {
                info!("🔍 ASKAI: AI response generated successfully: '{}'", response);
                Ok(response)
            },
            Err(e) => {
                error!("🔍 ASKAI: Error generating AI response: {:?}", e);
                Err(Error::Internal(format!("AI API error: {}", e)))
            }
        }
    }
}

// Helper function to send AI response to chat
async fn send_ai_response_to_chat(
    ctx: &RedeemHandlerContext<'_>,
    channel: &str,
    response: &str,
) -> Result<(), Error> {
    info!("🚀 ASKAI: Attempting to send AI response to chat channel: {}", channel);
    info!("🚀 ASKAI: Response to send: '{}'", response);
    
    // Make sure the channel name starts with a # prefix for Twitch IRC
    let channel_with_hash = if !channel.starts_with('#') {
        format!("#{}", channel)
    } else {
        channel.to_string()
    };
    
    info!("🚀 ASKAI: Using channel name with hash: {}", channel_with_hash);
    
    // Find a Twitch IRC credential to respond with
    let platform_mgr = &ctx.redeem_service.platform_manager;
    
    // Try several methods to find a suitable credential
    if let Some(active_cred) = &ctx.active_credential {
        info!("🚀 ASKAI: Using active credential ({}) from context to send message", active_cred.user_name);
        
        // Add more diagnostic info about this credential
        info!("🚀 ASKAI: Active credential details: user_id={}, platform={:?}, is_bot={}, is_broadcaster={}", 
            active_cred.user_id, active_cred.platform, active_cred.is_bot, active_cred.is_broadcaster);
        
        info!("🚀 ASKAI: Attempting to send message using credential: {} to channel: {}", 
              active_cred.user_name, channel_with_hash);
        
        // Use the proper channel format with # prefix
        match platform_mgr.send_twitch_irc_message(&active_cred.user_name, &channel_with_hash, response).await {
            Ok(_) => {
                info!("🚀 ASKAI: Successfully sent message using active credential to channel: {}", channel_with_hash);
                return Ok(());
            },
            Err(e) => {
                warn!("🚀 ASKAI: Failed to send message with active credential: {:?}", e);
                // Continue to try other methods
            }
        }
    } else {
        info!("🚀 ASKAI: No active credential found in context");
    }
    
    // If we get here, try to find a bot credential from the repository
    info!("🚀 ASKAI: Looking for a bot credential to send message");
    match ctx.redeem_service.credentials_repo
        .list_credentials_for_platform(&maowbot_common::models::platform::Platform::TwitchIRC)
        .await 
    {
        Ok(all_irc_creds) => {
            info!("🚀 ASKAI: Found {} Twitch IRC credentials", all_irc_creds.len());
            
            // Dump details about all credentials for debugging
            for (i, cred) in all_irc_creds.iter().enumerate() {
                info!("🚀 ASKAI: Credential #{}: user_name={}, is_bot={}, is_broadcaster={}", 
                      i, cred.user_name, cred.is_bot, cred.is_broadcaster);
            }
            
            // First, try with a known-working hard-coded credential
            // Try to use the credential "maowBot" to send the message
            for cred in &all_irc_creds {
                if cred.user_name.to_lowercase() == "maowbot" {
                    info!("🚀 ASKAI: Found known-working credential 'maowBot', trying it first");
                    match platform_mgr.send_twitch_irc_message("maowBot", &channel_with_hash, response).await {
                        Ok(_) => {
                            info!("🚀 ASKAI: Successfully sent message using 'maowBot' credential");
                            return Ok(());
                        },
                        Err(e) => {
                            warn!("🚀 ASKAI: Failed to send message with 'maowBot' credential: {:?}", e);
                        }
                    }
                    break;
                }
            }
            
            // Try bot credential next
            if let Some(bot_cred) = all_irc_creds.iter().find(|c| c.is_bot) {
                info!("🚀 ASKAI: Found bot credential: {}", bot_cred.user_name);
                match platform_mgr.send_twitch_irc_message(&bot_cred.user_name, &channel_with_hash, response).await {
                    Ok(_) => {
                        info!("🚀 ASKAI: Successfully sent message using bot credential");
                        return Ok(());
                    },
                    Err(e) => {
                        warn!("🚀 ASKAI: Failed to send message with bot credential: {:?}", e);
                    }
                }
            } else {
                info!("🚀 ASKAI: No bot credential found");
            }
            
            // Try broadcaster credential next
            if let Some(broadcaster_cred) = all_irc_creds.iter().find(|c| c.is_broadcaster) {
                info!("🚀 ASKAI: Found broadcaster credential: {}", broadcaster_cred.user_name);
                match platform_mgr.send_twitch_irc_message(&broadcaster_cred.user_name, &channel_with_hash, response).await {
                    Ok(_) => {
                        info!("🚀 ASKAI: Successfully sent message using broadcaster credential");
                        return Ok(());
                    },
                    Err(e) => {
                        warn!("🚀 ASKAI: Failed to send message with broadcaster credential: {:?}", e);
                    }
                }
            } else {
                info!("🚀 ASKAI: No broadcaster credential found");
            }
            
            // If all else fails, try any credential
            if !all_irc_creds.is_empty() {
                let first_cred = &all_irc_creds[0];
                info!("🚀 ASKAI: Using first available credential: {}", first_cred.user_name);
                match platform_mgr.send_twitch_irc_message(&first_cred.user_name, &channel_with_hash, response).await {
                    Ok(_) => {
                        info!("🚀 ASKAI: Successfully sent message using first available credential");
                        return Ok(());
                    },
                    Err(e) => {
                        warn!("🚀 ASKAI: Failed to send message with first available credential: {:?}", e);
                    }
                }
            } else {
                warn!("🚀 ASKAI: No credentials available at all");
            }
        },
        Err(e) => {
            error!("🚀 ASKAI: Failed to list credentials: {:?}", e);
        }
    }
    
    // If we reach here, we couldn't send the message with any credential
    warn!("🚀 ASKAI: Failed to find any suitable credential to send AI response");
    Err(Error::Internal("No credential available to send AI response".to_string()))
}

// Helper function to convert Twitch user ID string to UUID user ID
async fn get_user_from_twitch_id(
    ctx: &RedeemHandlerContext<'_>, 
    twitch_user_id: &str
) -> Result<User, Error> {
    // For Twitch redeems, we'll get the user ID from the user service directly
    // This is simpler than using the User API which doesn't have a direct method
    // for looking up users by platform ID
    
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
        // Get a Helix client instance for redemption management
        let helix_client_opt = ctx.redeem_service.platform_manager.get_twitch_client().await;
        
        // If we got a client, use it, otherwise log a warning
        if let Some(ref client) = helix_client_opt {
            info!("Using Helix client from platform manager");
        } else if ctx.helix_client.is_some() {
            info!("Using Helix client from context");
            // We'll use this in the next section
        } else {
            warn!("No Helix client available from any source");
        }

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

    // Log that we received the redeem and the input
    info!("Received askai redeem with input: {}", user_input);
    
    // Get the user from the Twitch ID
    let user = match get_user_from_twitch_id(ctx, &redemption.user_id).await {
        Ok(user) => {
            info!("Found user for AI redeem: {}", user.user_id);
            user
        },
        Err(e) => {
            error!("Failed to get user for AI redeem: {:?}", e);
            
            // Try to cancel the redemption since we can't process it
            // Get a Helix client for canceling the redemption
            let helix_client_opt = ctx.redeem_service.platform_manager.get_twitch_client().await;
            
            // First try with context client
            if let Some(client) = &ctx.helix_client {
                if let Err(cancel_err) = client
                    .update_redemption_status(
                        &redemption.broadcaster_id,
                        &redemption.reward.id,
                        &[&redemption.id],
                        "CANCELED",
                    )
                    .await
                {
                    warn!("Failed to cancel redemption using context client: {:?}", cancel_err);
                } else {
                    info!("Successfully canceled redemption using context client");
                }
            } else if let Some(client) = helix_client_opt {
                if let Err(cancel_err) = client
                    .update_redemption_status(
                        &redemption.broadcaster_id,
                        &redemption.reward.id,
                        &[&redemption.id],
                        "CANCELED",
                    )
                    .await
                {
                    warn!("Failed to cancel redemption using platform manager client: {:?}", cancel_err);
                } else {
                    info!("Successfully canceled redemption using platform manager client");
                }
            } else {
                warn!("No Helix client available to cancel redemption");
            }
            
            return Err(e);
        }
    };
    
    // Generate an AI response using real AI API
    let response = match generate_ai_response(ctx, user.user_id, user_input, None).await {
        Ok(resp) => {
            info!("Generated standard AI response: '{}'", resp);
            resp
        },
        Err(e) => {
            error!("Error generating AI response: {:?}", e);
            format!("Sorry, I couldn't generate a response: {}", e)
        }
    };
    
    // Send the response to chat with more detailed logging
    if let Some(broadcaster_login) = &redemption.broadcaster_login {
        info!("🚀 ASKAI: Trying to send AI response to broadcaster channel: {}", broadcaster_login);
        match send_ai_response_to_chat(ctx, broadcaster_login, &response).await {
            Ok(_) => {
                info!("🚀 ASKAI: Successfully sent AI response to chat channel: {}", broadcaster_login);
                
                // Try to send a follow-up confirmation message for debugging
                let fallback_msg = "[DEBUG] AI response was successfully sent by the askai redeem handler";
                // Use same channel hash format approach as the main function
                let login_with_hash = if !broadcaster_login.starts_with('#') {
                    format!("#{}", broadcaster_login)
                } else {
                    broadcaster_login.to_string()
                };
                
                match ctx.redeem_service.platform_manager.send_twitch_irc_message(
                    "maowBot",  // Most likely name of the bot
                    &login_with_hash,
                    fallback_msg
                ).await {
                    Ok(_) => info!("🚀 ASKAI: Sent confirmation message about successful AI response"),
                    Err(e) => warn!("🚀 ASKAI: Failed to send follow-up confirmation: {:?}", e),
                }
            },
            Err(e) => {
                error!("🚀 ASKAI: Failed to send AI response to chat: {:?}", e);
                
                // Try to send a fallback error message
                if let Ok(creds) = ctx.redeem_service.credentials_repo
                    .list_credentials_for_platform(&maowbot_common::models::platform::Platform::TwitchIRC)
                    .await
                {
                    if let Some(bot_cred) = creds.iter().find(|c| c.is_bot) {
                        let fallback_msg = format!("[ERROR] Failed to process AI redeem: {}", e);
                        // Make sure error message uses proper channel format
                        let login_with_hash = if !broadcaster_login.starts_with('#') {
                            format!("#{}", broadcaster_login)
                        } else {
                            broadcaster_login.to_string()
                        };
                        
                        if let Err(e2) = ctx.redeem_service.platform_manager.send_twitch_irc_message(
                            &bot_cred.user_name,
                            &login_with_hash,
                            &fallback_msg
                        ).await {
                            error!("🚀 ASKAI: Even fallback error message failed: {:?}", e2);
                        }
                    }
                }
            }
        }
    } else {
        error!("🚀 ASKAI: No broadcaster login found in redemption - can't send response");
    }
    
    // Try to mark the redemption as complete
    // Get a Helix client to fulfill the redemption
    let helix_client_opt = ctx.redeem_service.platform_manager.get_twitch_client().await;
    let broadcaster_id = &redemption.broadcaster_id;
    let reward_id = &redemption.reward.id;
    let redemption_id = &redemption.id;
    
    info!("Attempting to complete AI redeem");
    
    // First try with context client
    if let Some(client) = &ctx.helix_client {
        match client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[&redemption_id],
                "FULFILLED",
            )
            .await
        {
            Ok(_) => info!("Successfully marked redemption as fulfilled using context client"),
            Err(e) => {
                warn!("Failed to mark redemption as fulfilled using context client: {:?}", e);
                
                // Try fall back to platform manager client if context client fails
                if let Some(client2) = helix_client_opt {
                    match client2
                        .update_redemption_status(
                            broadcaster_id,
                            reward_id,
                            &[&redemption_id],
                            "FULFILLED",
                        )
                        .await 
                    {
                        Ok(_) => info!("Successfully marked redemption as fulfilled using platform manager client"),
                        Err(e2) => warn!("Failed to mark redemption as fulfilled using platform manager client: {:?}", e2)
                    }
                }
            }
        }
    } else if let Some(client) = helix_client_opt {
        // Fall back to platform manager client
        match client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[&redemption_id],
                "FULFILLED",
            )
            .await
        {
            Ok(_) => info!("Successfully marked redemption as fulfilled using platform manager client"),
            Err(e) => warn!("Failed to mark redemption as fulfilled using platform manager client: {:?}", e)
        }
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

            info!(
                "No user input provided for 'ask maow' redeem, canceling redemption"
            );

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

    // Log that we received the redeem and the input
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
    
    // Create a cat-like prompt for Maowbot
    let system_prompt = "You are Maowbot, a sassy and humorous cat-like AI. Respond with cat-like mannerisms, occasional 'meow' sounds, and a playful attitude. Your responses should be brief, funny, and slightly sarcastic while still being helpful. Limit responses to 1-2 sentences when possible.";
    
    // Generate an AI response with cat-like system prompt using real AI API
    let response = match generate_ai_response(ctx, user.user_id, user_input, Some(system_prompt)).await {
        Ok(resp) => {
            info!("Generated cat-like AI response");
            resp
        },
        Err(e) => {
            error!("Error generating cat-like AI response: {:?}", e);
            format!("Meow? *looks confused* Something went wrong with my cat brain. Try again later!")
        }
    };
    
    // Send the response to chat
    if let Some(broadcaster_login) = &redemption.broadcaster_login {
        // Use proper broadcaster_login
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

            info!(
                "No user input provided for 'ask ai with search' redeem, canceling redemption"
            );

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

    // Log that we received the redeem and the input
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
    
    // Create a search prompt 
    let system_prompt = "You are a helpful AI assistant that has the ability to search the web for information. For this request, respond as if you had searched for this information, providing a disclaimer that web search is not yet implemented. Format your response to include 'Search Results:' at the beginning.";
    
    // Generate an AI response with search prompt using real AI API
    let response = match generate_ai_response(ctx, user.user_id, user_input, Some(system_prompt)).await {
        Ok(resp) => {
            info!("Generated search AI response");
            resp
        },
        Err(e) => {
            error!("Error generating search AI response: {:?}", e);
            format!("Search Results: I couldn't perform a search due to a technical error. Please try again later.")
        }
    };
    
    // Send the response to chat
    if let Some(broadcaster_login) = &redemption.broadcaster_login {
        // Use proper broadcaster_login
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