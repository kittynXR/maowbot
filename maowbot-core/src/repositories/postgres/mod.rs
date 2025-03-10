use sqlx::{Pool, Postgres};
use crate::Error;
use async_trait::async_trait;

pub mod user;
pub mod platform_identity;
pub mod credentials;
pub mod analytics;
pub mod link_requests;
pub mod user_audit_log;
pub mod user_analysis;
pub mod bot_config;
pub mod platform_config;
pub mod commands;
pub mod command_usage;
pub mod redeems;
pub mod redeem_usage;

pub use user::UserRepository;
pub use platform_identity::PlatformIdentityRepository;
pub use credentials::PostgresCredentialsRepository;
pub use analytics::{PostgresAnalyticsRepository, AnalyticsRepo};
pub use link_requests::{PostgresLinkRequestsRepository, LinkRequestsRepository};
pub use user_audit_log::{PostgresUserAuditLogRepository, UserAuditLogRepository};
pub use user_analysis::{PostgresUserAnalysisRepository, UserAnalysisRepository};
pub use bot_config::{PostgresBotConfigRepository, BotConfigRepository};
pub use platform_config::{PostgresPlatformConfigRepository, PlatformConfigRepository};

