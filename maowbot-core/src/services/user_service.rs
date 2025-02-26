use std::sync::Arc;
use crate::Error;

use crate::auth::user_manager::{UserManager, DefaultUserManager};
use crate::models::{User, Platform};
use crate::repositories::postgres::user::UserRepo;
use crate::repositories::postgres::platform_identity::PlatformIdentityRepo;

/// The UserService adds some higher-level operations on top of the raw user_manager,
/// such as merging roles, etc.
pub struct UserService {
    pub user_manager: Arc<DefaultUserManager>,
    pub platform_identity_repo: Arc<dyn PlatformIdentityRepo + Send + Sync>,
}

impl UserService {
    pub fn new(
        user_manager: Arc<DefaultUserManager>,
        platform_identity_repo: Arc<dyn PlatformIdentityRepo + Send + Sync>,
    ) -> Self {
        Self {
            user_manager,
            platform_identity_repo,
        }
    }

    /// A convenience to unify roles for a user’s platform identity. If the identity
    /// doesn’t exist, we create it. We then do a union of existing roles + new roles.
    pub async fn unify_platform_roles(
        &self,
        user_id: uuid::Uuid,
        platform: Platform,
        new_roles: &[String],
    ) -> Result<(), Error> {
        // 1) Try to fetch the existing identity row
        if let Some(mut pid) = self.platform_identity_repo.get_by_user_and_platform(user_id, &platform).await? {
            // union
            let mut changed = false;
            for nr in new_roles {
                if !pid.platform_roles.contains(nr) {
                    pid.platform_roles.push(nr.clone());
                    changed = true;
                }
            }
            if changed {
                pid.last_updated = chrono::Utc::now();
                self.platform_identity_repo.update(&pid).await?;
            }
        } else {
            // create a brand new identity row with these roles
            let new_pi = crate::models::PlatformIdentity {
                platform_identity_id: uuid::Uuid::new_v4(),
                user_id,
                platform: platform.clone(),
                platform_user_id: "unknown".to_string(), // We only know partial data
                platform_username: "unknown".to_string(),
                platform_display_name: None,
                platform_roles: new_roles.to_vec(),
                platform_data: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                last_updated: chrono::Utc::now(),
            };
            self.platform_identity_repo.create(&new_pi).await?;
        }

        Ok(())
    }

    /// A wrapper around user_manager.get_or_create_user
    pub async fn get_or_create_user(
        &self,
        platform_name: &str,
        platform_user_id: &str,
        username: Option<&str>,
    ) -> Result<User, Error> {
        let platform = match platform_name {
            "discord" => Platform::Discord,
            "twitch_helix" => Platform::Twitch,
            "vrchat" => Platform::VRChat,
            "twitch-irc" => Platform::TwitchIRC,
            "twitch-eventsub" => Platform::TwitchEventSub,
            other => return Err(Error::Platform(format!("Unknown platform: {}", other))),
        };

        let user = self.user_manager
            .get_or_create_user(platform, platform_user_id, username)
            .await?;

        // Also ensure user_analysis exists
        let _analysis = self.user_manager
            .get_or_create_user_analysis(user.user_id)
            .await?;

        Ok(user)
    }

    /// Find a user by their global_username
    pub async fn find_user_by_global_username(
        &self,
        name: &str
    ) -> Result<User, Error> {
        let maybe = self.user_manager.user_repo
            .get_by_global_username(name).await?;
        if let Some(u) = maybe {
            Ok(u)
        } else {
            Err(Error::Platform(format!("No user with global_username='{}'", name)))
        }
    }
}