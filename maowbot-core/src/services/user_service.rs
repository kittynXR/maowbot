use std::sync::Arc;
use crate::Error;

use crate::auth::user_manager::{UserManager, DefaultUserManager};
use crate::models::{User, Platform};

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
}