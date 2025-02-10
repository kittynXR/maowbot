// src/repositories/mod.rs

use async_trait::async_trait;
use crate::Error;
use crate::models::{Platform, PlatformIdentity, PlatformCredential};

/// A generic Repository trait (if you still want it), typed by T.
#[async_trait]
pub trait Repository<T> {
    async fn create(&self, item: &T) -> Result<(), Error>;
    async fn get(&self, id: &str) -> Result<Option<T>, Error>;
    async fn update(&self, item: &T) -> Result<(), Error>;
    async fn delete(&self, id: &str) -> Result<(), Error>;
}

/// If you still keep a dedicated trait for platform identities:
#[async_trait]
pub trait PlatformIdentityRepository {
    // Possibly you want to use Uuid instead of &str for user_id as well
    // but if your code references it as &str, you can keep this shape.
    // The actual implementation can parse or store it as needed.
    fn get_by_platform(
        &self,
        platform: Platform,
        platform_user_id: &str
    ) -> impl std::future::Future<Output = Result<Option<PlatformIdentity>, Error>> + Send;

    fn get_all_for_user(
        &self,
        user_id: &str
    ) -> impl std::future::Future<Output = Result<Vec<PlatformIdentity>, Error>> + Send;
}

pub use postgres::credentials::{
    CredentialsRepository,
    PostgresCredentialsRepository,
};

pub use postgres::bot_config::{BotConfigRepository, PostgresBotConfigRepository};
pub use postgres::platform_config::{PlatformConfigRepository, PostgresPlatformConfigRepository};

// The rest of your repository modules:
pub mod postgres;