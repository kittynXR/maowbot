use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn, trace};
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
    trace!("Running Discord live role check for all users...");
    
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
        
        trace!("Processing live role for guild {}", guild_id_str);
        
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

        // FIRST PASS: Check users WHO ALREADY HAVE THE ROLE to see if they should keep it
        trace!("FIRST PASS: Checking users who have the live role...");
        
        // Get guild members with the role
        match http.guild_members(guild_id).limit(1000).await {
            Ok(members_response) => {
                let members = match members_response.model().await {
                    Ok(m) => m,
                    Err(e) => {
                        warn!("Failed to parse guild members response: {:?}", e);
                        continue;
                    }
                };
                
                // First, process only members WITH the role
                for member in members.iter().filter(|m| m.roles.contains(&role_id)) {
                    let user_id = member.user.id;
                    
                    trace!("Found user {} with the live role", user_id);
                    
                    // Check if they're actually streaming
                    let mut is_streaming = false;
                    
                    // Try to fetch their activities directly using HTTP
                    // For each user with the role, check their presence
                    match http.guild_member(guild_id, user_id).await {
                        Ok(member_response) => {
                            match member_response.model().await {
                                Ok(member_detail) => {
                                    trace!("Checking streaming status for user with role: {}", user_id);
                                    
                                    // Member detail doesn't directly have presence data
                                    // Check in the cache for presence data instead
                                    if let Some(presence) = cache.presence(guild_id, user_id) {
                                        trace!("Found presence data for user {} in cache", user_id);
                                        
                                        // Check all activities
                                        for (idx, activity) in presence.activities().iter().enumerate() {
                                            trace!("User {} activity {}: type={:?}, name={}, url={:?}",
                                                  user_id, idx, activity.kind, activity.name, activity.url);
                                            
                                            if activity.kind == ActivityType::Streaming {
                                                if let Some(url) = &activity.url {
                                                    if url.contains("twitch.tv") {
                                                        is_streaming = true;
                                                        trace!("User {} is streaming on Twitch", user_id);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        trace!("No presence data found in cache for user {}", user_id);
                                        
                                        // Try one last approach - look for the user in active users
                                        // We'll check if they have a streaming activity in any guild
                                        trace!("Checking for user's streaming activity in any guild...");
                                        
                                        // We can't list all guilds directly, so we'll just check the current guild
                                        let mut found_streaming = false;
                                        
                                        // Check the current guild since we know that one
                                        if let Some(presence) = cache.presence(guild_id, user_id) {
                                            trace!("Found presence for user {} in guild {}", user_id, guild_id);
                                            
                                            // Check all activities
                                            for (idx, activity) in presence.activities().iter().enumerate() {
                                                trace!("Found activity {}: type={:?}, name={}, url={:?}",
                                                      idx, activity.kind, activity.name, activity.url);
                                                
                                                if activity.kind == ActivityType::Streaming {
                                                    if let Some(url) = &activity.url {
                                                        if url.contains("twitch.tv") {
                                                            found_streaming = true;
                                                            is_streaming = true;
                                                            trace!("User {} is streaming on Twitch in guild {}", user_id, guild_id);
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            trace!("No presence data found for user {} in guild {}", user_id, guild_id);
                                        }
                                        
                                        if !found_streaming {
                                            trace!("No streaming activity found for user {} in any guild", user_id);
                                        }
                                    }
                                    
                                    // If not streaming, remove the role
                                    if !is_streaming {
                                        trace!("User {} has the live role but isn't streaming - removing role", user_id);
                                        
                                        if let Err(e) = http.remove_guild_member_role(guild_id, user_id, role_id).await {
                                            warn!("Failed to remove live role from non-streaming user: {:?}", e);
                                        } else {
                                            info!("Successfully removed live role from user {}", user_id);
                                        }
                                    } else {
                                        trace!("User {} is streaming - keeping live role", user_id);
                                    }
                                },
                                Err(e) => {
                                    warn!("Failed to parse member detail for {}: {:?}", user_id, e);
                                }
                            }
                        },
                        Err(e) => {
                            warn!("Failed to fetch member detail for {}: {:?}", user_id, e);
                        }
                    }
                }
                
                // SECOND PASS: Check all members to find anyone streaming who doesn't have the role
                trace!("SECOND PASS: Checking all users for streaming activity...");
                
                for member in &members {
                    let user_id = member.user.id;
                    let has_role = member.roles.contains(&role_id);
                    
                    // Skip users who already have the role (we handled them in the first pass)
                    if has_role {
                        continue;
                    }
                    
                    // Check if they're streaming
                    let mut is_streaming = false;
                    
                    // First try the cache
                    if let Some(presence) = cache.presence(guild_id, user_id) {
                        for activity in presence.activities() {
                            if activity.kind == ActivityType::Streaming {
                                if let Some(url) = &activity.url {
                                    if url.contains("twitch.tv") {
                                        is_streaming = true;
                                        info!("Found user {} streaming without role", user_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    
                    // If streaming but doesn't have the role, add it
                    if is_streaming {
                        info!("Adding live role to streaming user {} who doesn't have it", user_id);
                        
                        if let Err(e) = http.add_guild_member_role(guild_id, user_id, role_id).await {
                            warn!("Failed to add live role to streaming user: {:?}", e);
                        } else {
                            info!("Successfully added live role to user {}", user_id);
                        }
                    }
                }
            },
            Err(e) => {
                warn!("Failed to get guild members for guild {}: {:?}", guild_id_str, e);
                continue;
            }
        }
    }
    
    trace!("Discord live role check completed");
    Ok(())
}

/// Spawns a background task that periodically checks streaming status
/// and updates live roles accordingly
pub fn spawn_discord_live_role_task(
    discord_platform: Arc<DiscordPlatform>,
    discord_repo: Arc<dyn DiscordRepository + Send + Sync>,
) -> tokio::task::JoinHandle<()> {
    info!("Starting periodic Discord live role task");
    
    tokio::spawn(async move {
        // Check immediately at startup
        info!("Running initial Discord live role check");
        if let Err(e) = check_all_streaming_status(&discord_platform, &discord_repo).await {
            error!("Initial Discord live role check failed: {:?}", e);
        } else {
            info!("Initial Discord live role check completed successfully");
        }
        
        // Then check periodically every 60 seconds for quicker role updates
        let mut interval = interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            trace!("Running periodic Discord live role check");
            if let Err(e) = check_all_streaming_status(&discord_platform, &discord_repo).await {
                error!("Periodic Discord live role check failed: {:?}", e);
            } else {
                trace!("Periodic Discord live role check completed successfully");
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
    
    // First try to find a Discord instance directly from active runtimes
    let mut discord_platform_opt = None;
    
    {
        let runtimes = platform_manager.active_runtimes.lock().await;
        for ((platform, _), handle) in runtimes.iter() {
            if platform == "discord" && handle.discord_instance.is_some() {
                discord_platform_opt = handle.discord_instance.clone();
                break;
            }
        }
    }
    
    // If we didn't find an active Discord instance directly, try the traditional way
    if discord_platform_opt.is_none() {
        // Try to find accounts via credentials as fallback
        let accounts = find_active_discord_account(platform_manager).await?;
        
        if accounts.is_empty() {
            warn!("No active Discord accounts found for live role management at startup");
            return Ok(());
        }
        
        // Use the first available Discord account
        let account_name = &accounts[0];
        
        match platform_manager.get_discord_instance(account_name).await {
            Ok(discord) => discord_platform_opt = Some(discord),
            Err(e) => {
                warn!("Could not get Discord instance for account '{}' at startup: {}", account_name, e);
                return Err(Error::Platform(format!("Could not get Discord instance for startup verification: {}", e)));
            }
        }
    }
    
    // Now proceed with the Discord instance we found
    if let Some(discord_platform) = discord_platform_opt {
        // At this point, Discord should have connected and started receiving events
        info!("Discord instance ready for checking live roles");
        
        // Get a direct reference to the cache to check if we have presence data
        if let Some(cache) = &discord_platform.cache {
            info!("Cache is available, getting live roles to check guilds...");
            
            // First get the configured live roles - this gives us the guild IDs we care about
            match discord_repo.list_live_roles().await {
                Ok(live_roles) => {
                    info!("Found {} configured live roles", live_roles.len());
                    
                    let mut total_presences = 0;
                    let mut streaming_count = 0;
                    
                    // Check each guild with a live role
                    for live_role in &live_roles {
                        let guild_id_str = &live_role.guild_id;
                        info!("Checking presence data for guild {}", guild_id_str);
                        
                        // Parse the guild ID
                        let guild_id_u64 = match guild_id_str.parse::<u64>() {
                            Ok(id) => id,
                            Err(e) => {
                                warn!("Invalid guild ID format: {}", e);
                                continue;
                            }
                        };
                        
                        let guild_id = Id::<GuildMarker>::new(guild_id_u64);
                        
                        // We can't get all presences directly, but we can check members
                        info!("Directly checking guild status for guild {}", guild_id);
                        
                        // Log cache statistics 
                        // We can't directly count members, but we can check if the guild exists in cache
                        if cache.guild(guild_id).is_some() {
                            info!("Guild {} exists in cache", guild_id);
                        } else {
                            info!("Guild {} not found in cache", guild_id);
                        }
                    }
                    
                    info!("Cache check complete. Active cache with {} live role guilds", live_roles.len());
                },
                Err(e) => {
                    warn!("Failed to list live roles: {}", e);
                }
            }
        } else {
            warn!("Discord cache not available");
        }
        
        // Run the streaming status check using this Discord platform
        info!("Running streaming verification check...");
        if let Err(e) = check_all_streaming_status(&discord_platform, discord_repo).await {
            error!("Startup Discord live role verification failed: {:?}", e);
            return Err(e);
        }
        
        info!("Startup verification of Discord live roles completed successfully");
        Ok(())
    } else {
        warn!("No Discord instances available for live role verification at startup");
        Ok(())
    }
}

/// Creates a task that performs a one-time verification of all users' streaming status
/// at bot startup and assigns/removes live roles accordingly.
pub fn spawn_discord_live_role_startup_task(
    platform_manager: Arc<PlatformManager>,
    discord_repo: Arc<dyn DiscordRepository + Send + Sync>,
) -> tokio::task::JoinHandle<()> {
    info!("Starting Discord live role startup verification task");
    
    // This task just needs to run once after a delay to ensure Discord connections are established
    tokio::spawn(async move {
        // Give Discord connections time to establish - increased to 45 seconds
        // This allows more time for the cache to fill with presence data
        info!("Waiting for Discord connections to establish before checking live roles (45 seconds)...");
        tokio::time::sleep(Duration::from_secs(45)).await;
        
        info!("Running one-time Discord live role verification task at startup");
        if let Err(e) = verify_streaming_status_at_startup(&platform_manager, &discord_repo).await {
            error!("Discord live role startup verification task failed: {:?}", e);
        } else {
            info!("Discord live role startup verification task completed successfully");
        }
        
        // Run multiple follow-up checks to ensure we don't miss anyone
        // First follow-up after 1 minute
        tokio::time::sleep(Duration::from_secs(60)).await;
        info!("Running first follow-up check...");
        if let Err(e) = verify_streaming_status_at_startup(&platform_manager, &discord_repo).await {
            error!("First follow-up check failed: {:?}", e);
        }
        
        // Second follow-up after another minute
        tokio::time::sleep(Duration::from_secs(60)).await;
        info!("Running second follow-up check...");
        if let Err(e) = verify_streaming_status_at_startup(&platform_manager, &discord_repo).await {
            error!("Second follow-up check failed: {:?}", e);
        }
    })
}

/// Helper function to find active Discord accounts
async fn find_active_discord_account(
    platform_manager: &crate::platforms::manager::PlatformManager,
) -> Result<Vec<String>, Error> {
    let mut active_accounts = Vec::new();
    
    // First check directly in active runtimes
    let runtimes = platform_manager.active_runtimes.lock().await;
    for ((platform, user_id), handle) in runtimes.iter() {
        if platform == "discord" && handle.discord_instance.is_some() {
            // Get the username for this user_id
            let creds_repo = &platform_manager.credentials_repo;
            if let Ok(cred) = creds_repo.get_credentials(&Platform::Discord, uuid::Uuid::parse_str(user_id).unwrap_or_default()).await {
                if let Some(credential) = cred {
                    active_accounts.push(credential.user_name);
                    continue;
                }
            }
            
            // Fallback: just use the user_id as the account name if we have a discord instance
            active_accounts.push(user_id.clone());
        }
    }
    
    // If we didn't find any active instances, fall back to checking credentials
    if active_accounts.is_empty() {
        // Check autostart config for Discord accounts
        let creds_repo = &platform_manager.credentials_repo;
        
        // Try to get credentials for Discord platform
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
    }
    
    Ok(active_accounts)
}
