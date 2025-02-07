// src/repositories/mod.rs
use async_trait::async_trait;
use crate::Error;
use crate::models::{Platform, PlatformIdentity, PlatformCredential};

// Trait definitions
#[async_trait]
pub trait Repository<T> {
    async fn create(&self, item: &T) -> Result<(), Error>;
    async fn get(&self, id: &str) -> Result<Option<T>, Error>;
    async fn update(&self, item: &T) -> Result<(), Error>;
    async fn delete(&self, id: &str) -> Result<(), Error>;
}

#[async_trait]
pub trait PlatformIdentityRepository {
    fn get_by_platform(&self, platform: Platform, platform_user_id: &str)
                       -> impl std::future::Future<Output = Result<Option<PlatformIdentity>, Error>> + Send;

    fn get_all_for_user(&self, user_id: &str)
                        -> impl std::future::Future<Output = Result<Vec<PlatformIdentity>, Error>> + Send;
}

pub use postgres::credentials::{
    CredentialsRepository,
    PostgresCredentialsRepository,
};

pub use postgres::bot_config::{BotConfigRepository, PostgresBotConfigRepository};
pub use postgres::auth_config::{AuthConfigRepository, PostgresAuthConfigRepository};

pub mod postgres;