use std::sync::Arc;
use crate::Error;

use crate::auth::user_manager::{UserManager, DefaultUserManager};
use crate::models::{User, Platform};
use crate::repositories::postgres::user::UserRepo;

pub struct UserService {
    user_manager: Arc<DefaultUserManager>,
}

impl UserService {
    pub fn new(user_manager: Arc<DefaultUserManager>) -> Self {
        Self { user_manager }
    }

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

        // pass user.user_id by value (not &str):
        let _analysis = self.user_manager
            .get_or_create_user_analysis(user.user_id)
            .await?;

        Ok(user)
    }

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