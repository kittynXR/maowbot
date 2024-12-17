use sqlx::{Pool, Sqlite};
use crate::Error;
use async_trait::async_trait;

pub mod user;
pub mod platform_identity;
mod credentials;

pub use self::user::UserRepository;
pub use self::platform_identity::PlatformIdentityRepository;
pub use self::credentials::SqliteCredentialsRepository;