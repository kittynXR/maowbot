use async_trait::async_trait;
use sqlx::{Pool, Sqlite};
use crate::Error;
use crate::models::{PlatformIdentity, Platform};

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

// Module declarations
pub mod sqlite;  // Only need sqlite module here since implementations will be there