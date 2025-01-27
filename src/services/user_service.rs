use std::sync::Arc;
use crate::Error;

// ADD THIS:
use crate::auth::user_manager::UserManager;  // so the trait is in scope

use crate::auth::user_manager::DefaultUserManager;
use crate::models::User;

// (remove `tokio::sync::Mutex` and `tracing::error` if unused)

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
        // ... same code ...
        let platform = match platform_name {
            "discord" => crate::models::Platform::Discord,
            "twitch"  => crate::models::Platform::Twitch,
            "vrchat"  => crate::models::Platform::VRChat,
            other => return Err(Error::Platform(format!("Unknown platform: {}", other))),
        };

        // Now calls the trait method correctly
        let user = self.user_manager
            .get_or_create_user(platform, platform_user_id, username)
            .await?;

        let _analysis = self.user_manager
            .get_or_create_user_analysis(&user.user_id)
            .await?;

        Ok(user)
    }
}
