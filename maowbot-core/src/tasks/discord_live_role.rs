use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use std::collections::HashSet;

use twilight_model::{
    gateway::presence::ActivityType,
    id::{marker::{GuildMarker, RoleMarker, UserMarker}, Id},
};

use crate::Error;
use crate::platforms::discord::DiscordPlatform;
use crate::platforms::manager::PlatformManager;
use maowbot_common::models::platform::{Platform, PlatformCredential};
use maowbot_common::traits::repository_traits::DiscordRepository;

/// Checks streaming status for all users in guilds with live roles configured
/// Adds or removes the configured role based on streaming status
pub async fn check_all_streaming_status(
    discord_platform: &DiscordPlatform,
    discord_repo: &Arc<dyn DiscordRepository + Send + Sync>,
) -> Result<(), Error> {
    info!("Running Discord live role check for all users...");
    
    // Get all configured live roles
    let live_roles = match discord_repo.list_live_roles().await {
        Ok(roles) => roles,
        Err(e) => {
            error!("Failed to list live roles: {}", e);
            return Err(Error::Internal(format!("Database error: {}", e)));
        }
    };
    
    if live_roles.is_empty() {
        debug!("No live roles configured, skipping check");
        return Ok(());
    }
    
    // Get the HTTP client and cache - we use both for different operations
    let http = match &discord_platform.http {
        Some(http) => http,
        None => {
            warn!("Discord HTTP client not available");
            return Ok(());
        }
    };
    
    let cache = match &discord_platform.cache {
        Some(cache) => cache,
        None => {
            warn!("Discord cache not available");
            return Ok(());
        }
    };
    
    // Process each guild with a live role
    for live_role in live_roles {
        let guild_id_str = &live_role.guild_id;
        let role_id_str = &live_role.role_id;
        
        debug!("Processing live role for guild {}", guild_id_str);
        
        // Parse the guild and role IDs
        let guild_id_u64 = match guild_id_str.parse::<u64>() {
            Ok(id) => id,
            Err(e) => {
                warn!("Invalid guild ID format: {}", e);
                continue;
            }
        };
        
        let role_id_u64 = match role_id_str.parse::<u64>() {
            Ok(id) => id,
            Err(e) => {
                warn!("Invalid role ID format: {}", e);
                continue;
            }
        };
        
        let guild_id = Id::<GuildMarker>::new(guild_id_u64);
        let role_id = Id::<RoleMarker>::new(role_id_u64);
        
        // Use the HTTP API to get guild members directly
        match http.guild_members(guild_id).limit(1000).await {
            Ok(members_response) => {
                let members = match members_response.model().await {
                    Ok(m) => m,
                    Err(e) => {
                        warn!("Failed to parse guild members response: {:?}", e);
                        continue;
                    }
                };
                
                // Process each member 
                for member in members {
                    let user_id = member.user.id;
                    let has_role = member.roles.contains(&role_id);
                    
                    // Simplify the streaming detection since we're having issues with the API
                    // We'll use the cache instead of trying to get presence via HTTP
                    let mut is_streaming = false;
                    
                    // Look for this member's presence in the cache
                    debug!("Checking cache for presence of user {} in guild {}", user_id, guild_id);
                    if let Some(presence) = cache.presence(guild_id, user_id) {
                        let activities = presence.activities();
                        debug!("Found {} activities for user {} in cache", activities.len(), user_id);
                        
                        for (idx, activity) in activities.iter().enumerate() {
                            // Log each activity to help debug
                            debug!("  Activity {}: type={:?}, name={}, url={:?}", 
                                  idx, activity.kind, activity.name, activity.url);
                            
                            // Direct field access - no method calls
                            if activity.kind == ActivityType::Streaming {
                                // URL is an Option<String> field
                                if let Some(url) = &activity.url {
                                    if url.contains("twitch.tv") {
                                        is_streaming = true;
                                        debug!("User {} is streaming on Twitch", user_id);
                                        break;
                                    }
                                }
                            }
                        }
                        
                        if !is_streaming {
                            debug!("User {} is not streaming (no streaming activity found in cache)", user_id);
                        }
                    } else {
                            // No presence data found in cache
                        if has_role {
                            // If they have the role but no presence data, they're likely not streaming
                            // This is a key case to handle - they have the role but shouldn't anymore
                            info!("No presence data found for user {} in guild {}, but they have the role - will remove it", 
                                 user_id, guild_id);
                            // We'll keep is_streaming = false to remove the role
                        } else {
                            debug!("No presence data found for user {} in guild {} and they don't have the role", 
                                  user_id, guild_id);
                        }
                    }
                    
                    // Take appropriate action based on streaming status and role
                    if is_streaming && !has_role {
                        // User is streaming but doesn't have the role - add it
                        info!("Periodic check: Adding live role {} to streaming user {} in guild {}", 
                              role_id, user_id, guild_id);
                        
                        if let Err(e) = http.add_guild_member_role(guild_id, user_id, role_id).await {
                            warn!("Failed to add live role to streaming user: {:?}", e);
                        } else {
                            info!("Successfully added live role to user {}", user_id);
                        }
                    } else if !is_streaming && has_role {
                        // User has the role but isn't streaming - remove it
                        info!("Periodic check: Removing live role {} from non-streaming user {} in guild {}", 
                              role_id, user_id, guild_id);
                        
                        if let Err(e) = http.remove_guild_member_role(guild_id, user_id, role_id).await {
                            warn!("Failed to remove live role from non-streaming user: {:?}", e);
                        } else {
                            info!("Successfully removed live role from user {}", user_id);
                        }
                    } else {
                        // Status matches role - no action needed
                        debug!("Periodic check: No role change needed for user {}: is_streaming={}, has_role={}", 
                               user_id, is_streaming, has_role);
                    }
                }
            },
            Err(e) => {
                warn!("Failed to get guild members for guild {}: {:?}", guild_id_str, e);
                continue;
            }
        }
    }
    
    info!("Discord live role check completed");
    Ok(())
}

/// Spawns a background task that periodically checks streaming status
/// and updates live roles accordingly
pub fn spawn_discord_live_role_task(
    discord_platform: Arc<DiscordPlatform>,
    discord_repo: Arc<dyn DiscordRepository + Send + Sync>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Check immediately at startup
        if let Err(e) = check_all_streaming_status(&discord_platform, &discord_repo).await {
            error!("Initial Discord live role check failed: {:?}", e);
        }
        
        // Then check periodically every 2 minutes
        let mut interval = interval(Duration::from_secs(120));
        
        loop {
            interval.tick().await;
            if let Err(e) = check_all_streaming_status(&discord_platform, &discord_repo).await {
                error!("Periodic Discord live role check failed: {:?}", e);
            }
        }
    })
}

/// Performs a check of all streaming statuses at startup
/// This function can be called when the bot starts up to ensure
/// live roles are correctly assigned from the beginning
pub async fn verify_streaming_status_at_startup(
    platform_manager: &crate::platforms::manager::PlatformManager,
    discord_repo: &Arc<dyn DiscordRepository + Send + Sync>,
) -> Result<(), Error> {
    info!("Performing startup verification of Discord live roles...");
    
    // Find the first active Discord account to use for role management
    let accounts = find_active_discord_account(platform_manager).await?;
    
    if accounts.is_empty() {
        warn!("No active Discord accounts found for live role management at startup");
        return Ok(());
    }
    
    // Use the first available Discord account
    let account_name = &accounts[0];
    
    match platform_manager.get_discord_instance(account_name).await {
        Ok(discord_platform) => {
            // Run the streaming status check using this Discord platform
            if let Err(e) = check_all_streaming_status(&discord_platform, discord_repo).await {
                error!("Startup Discord live role verification failed: {:?}", e);
                return Err(e);
            }
            
            info!("Startup verification of Discord live roles completed successfully");
            Ok(())
        },
        Err(e) => {
            warn!("Could not get Discord instance for account '{}' at startup: {}", account_name, e);
            Err(Error::Platform(format!("Could not get Discord instance for startup verification: {}", e)))
        }
    }
}

/// Creates a task that performs a one-time verification of all users' streaming status
/// at bot startup and assigns/removes live roles accordingly.
pub fn spawn_discord_live_role_startup_task(
    platform_manager: Arc<PlatformManager>,
    discord_repo: Arc<dyn DiscordRepository + Send + Sync>,
) -> tokio::task::JoinHandle<()> {
    // This task just needs to run once after a short delay to ensure Discord connections are established
    tokio::spawn(async move {
        // Give Discord connections time to establish
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        info!("Running one-time Discord live role verification task at startup");
        if let Err(e) = verify_streaming_status_at_startup(&platform_manager, &discord_repo).await {
            error!("Discord live role startup verification task failed: {:?}", e);
        } else {
            info!("Discord live role startup verification task completed successfully");
        }
    })
}

/// Helper function to find active Discord accounts
async fn find_active_discord_account(
    platform_manager: &crate::platforms::manager::PlatformManager,
) -> Result<Vec<String>, Error> {
    let mut active_accounts = Vec::new();
    
    // Check autostart config for Discord accounts
    let creds_repo = &platform_manager.credentials_repo;
    
    // First try to get credentials for Discord platform
    let discord_creds = match creds_repo.list_credentials_for_platform(&Platform::Discord).await {
        Ok(creds) => creds,
        Err(e) => {
            warn!("Failed to get Discord credentials: {}", e);
            return Ok(Vec::new());
        }
    };
    
    // For each credential, add the username to our list
    for cred in discord_creds {
        active_accounts.push(cred.user_name);
    }
    
    Ok(active_accounts)
}