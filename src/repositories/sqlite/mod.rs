// src/repositories/sqlite/mod.rs

use sqlx::{Pool, Sqlite};
use crate::Error;
use async_trait::async_trait;

pub mod user;
pub mod platform_identity;
pub mod credentials;
pub mod analytics;

pub mod link_requests;
pub mod user_audit_log;
pub mod user_analysis;

pub use user_analysis::{SqliteUserAnalysisRepository, UserAnalysisRepository};

pub use self::user::UserRepository;
pub use self::platform_identity::PlatformIdentityRepository;
pub use self::credentials::SqliteCredentialsRepository;
pub use self::link_requests::{SqliteLinkRequestsRepository, LinkRequestsRepository};
pub use self::user_audit_log::{SqliteUserAuditLogRepository, UserAuditLogRepository};
