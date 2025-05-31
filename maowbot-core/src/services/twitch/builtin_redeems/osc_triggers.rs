use tracing::{info, warn, error};
use crate::Error;
use crate::services::twitch::redeem_service::RedeemHandlerContext;
use crate::platforms::twitch::requests::channel_points::Redemption;
use maowbot_common::traits::api::OscApi;
use uuid::Uuid;

/// Handle the cat trap OSC toggle redeem
pub async fn handle_cattrap_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
) -> Result<(), Error> {
    info!(
        "Builtin 'cat trap' redeem triggered for user_id={} reward='{}'",
        redemption.user_id, redemption.reward.title
    );

    // Mark redemption as complete
    if let Some(client) = &ctx.helix_client {
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;

        client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[redemption_id],
                "FULFILLED",
            )
            .await?;
    }
    
    // Get the platform manager's plugin manager to access OSC
    let platform_manager = &ctx.redeem_service.platform_manager;
    
    if let Some(plugin_manager) = platform_manager.plugin_manager() {
        // Use the existing OSC send method from the plugin manager
        match plugin_manager.osc_send_avatar_parameter_bool("CatTrap", true).await {
            Ok(_) => {
                info!("Successfully activated cat trap toggle");
                
                // Schedule toggle off after 30 seconds
                let plugin_manager_clone = plugin_manager.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    if let Err(e) = plugin_manager_clone.osc_send_avatar_parameter_bool("CatTrap", false).await {
                        error!("Failed to deactivate cat trap toggle: {}", e);
                    } else {
                        info!("Deactivated cat trap toggle after 30 seconds");
                    }
                });
            }
            Err(e) => {
                error!("Failed to activate cat trap toggle: {}", e);
                // Don't fail the redeem if OSC fails
            }
        }
    } else {
        warn!("Plugin manager not available for OSC toggle");
    }

    Ok(())
}

/// Generic handler for OSC toggle redeems that uses the database configuration
pub async fn handle_generic_osc_toggle(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
    redeem_id: Uuid,
) -> Result<(), Error> {
    info!(
        "Generic OSC toggle redeem triggered for user_id={} reward='{}' redeem_id={}",
        redemption.user_id, redemption.reward.title, redeem_id
    );

    // Mark redemption as complete
    if let Some(client) = &ctx.helix_client {
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;

        client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[redemption_id],
                "FULFILLED",
            )
            .await?;
    }
    
    // Get the platform manager's plugin manager to access OSC toggle service
    let platform_manager = &ctx.redeem_service.platform_manager;
    
    if let Some(plugin_manager) = platform_manager.plugin_manager() {
        // Get or create the user to ensure they exist in our database
        let user = match ctx.redeem_service.user_service.get_or_create_user(
            "twitch-eventsub",
            &redemption.user_id,
            redemption.user_name.as_deref()
        ).await {
            Ok(user) => user,
            Err(e) => {
                error!("Failed to get/create user for OSC toggle: {}", e);
                return Ok(());
            }
        };
        
        let user_uuid = user.user_id;
        
        // Use the OSC toggle service to activate the toggle
        match plugin_manager.osc_activate_toggle(redeem_id, user_uuid).await {
            Ok(_) => {
                info!("Successfully activated OSC toggle for redeem {}", redeem_id);
            }
            Err(e) => {
                error!("Failed to activate OSC toggle: {}", e);
                // Don't fail the redeem if OSC fails
            }
        }
    } else {
        warn!("Plugin manager not available for OSC toggle");
    }

    Ok(())
}

/// Handle the pillo OSC toggle redeem
pub async fn handle_pillo_redeem(
    ctx: &RedeemHandlerContext<'_>,
    redemption: &Redemption,
) -> Result<(), Error> {
    info!(
        "Builtin 'pillo' redeem triggered for user_id={} reward='{}'",
        redemption.user_id, redemption.reward.title
    );

    // Mark redemption as complete
    if let Some(client) = &ctx.helix_client {
        let broadcaster_id = &redemption.broadcaster_id;
        let reward_id = &redemption.reward.id;
        let redemption_id = &redemption.id;

        client
            .update_redemption_status(
                broadcaster_id,
                reward_id,
                &[redemption_id],
                "FULFILLED",
            )
            .await?;
    }
    
    // Get the platform manager's plugin manager to access OSC
    let platform_manager = &ctx.redeem_service.platform_manager;
    
    if let Some(plugin_manager) = platform_manager.plugin_manager() {
        // Use the existing OSC send method from the plugin manager
        match plugin_manager.osc_send_avatar_parameter_bool("Pillo", true).await {
            Ok(_) => {
                info!("Successfully activated pillo toggle");
                
                // Schedule toggle off after 7 seconds (as requested)
                let plugin_manager_clone = plugin_manager.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(7)).await;
                    if let Err(e) = plugin_manager_clone.osc_send_avatar_parameter_bool("Pillo", false).await {
                        error!("Failed to deactivate pillo toggle: {}", e);
                    } else {
                        info!("Deactivated pillo toggle after 7 seconds");
                    }
                });
            }
            Err(e) => {
                error!("Failed to activate pillo toggle: {}", e);
                // Don't fail the redeem if OSC fails
            }
        }
    } else {
        warn!("Plugin manager not available for OSC toggle");
    }

    Ok(())
}