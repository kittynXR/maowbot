use async_trait::async_trait;
use sqlx::{Pool, Sqlite};
use crate::Error;
use crate::models::{PlatformIdentity, Platform};

#[async_trait]
pub trait Repository<T> {
    async fn create(&self, item: &T) -> Result<(), Error>;
    async fn get(&self, id: &str) -> Result<Option<T>, Error>;
    async fn update(&self, item: &T) -> Result<(), Error>;
    async fn delete(&self, id: &str) -> Result<(), Error>;
}

pub trait PlatformIdentityRepository: Repository<PlatformIdentity> {
    async fn get_by_platform(&self, platform: Platform, platform_user_id: &str)
                             -> Result<Option<PlatformIdentity>, Error>;
    async fn get_all_for_user(&self, user_id: &str)
                              -> Result<Vec<PlatformIdentity>, Error>;
}

pub mod user;
mod platform_identity;

pub use user::UserRepository;