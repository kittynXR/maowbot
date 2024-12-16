// src/repositories/sqlite/mod.rs
// use sqlx::{Pool, Sqlite};
// use crate::Error;
// use async_trait::async_trait;
use crate::repositories::{user, platform_identity};

pub use self::user::UserRepository;
pub use self::platform_identity::PlatformIdentityRepository;