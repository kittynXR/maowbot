use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;
use async_trait::async_trait;

use dashmap::DashMap;

use crate::Error;
use crate::models::{User, Platform, PlatformIdentity};
use crate::models::user_analysis::UserAnalysis;
use crate::repositories::Repository;
use crate::repositories::postgres::{
    user::UserRepository,
    platform_identity::PlatformIdentityRepository,
    user_analysis::{UserAnalysisRepository, PostgresUserAnalysisRepository},
};
use crate::repositories::postgres::platform_identity::PlatformIdentityRepo;
use crate::repositories::postgres::user::UserRepo;

#[async_trait]
pub trait UserManager: Send + Sync {
    /// Looks up or creates a user record for (platform, platform_user_id).
    async fn get_or_create_user(
        &self,
        platform: Platform,
        platform_user_id: &str,
        platform_username: Option<&str>,
    ) -> Result<User, Error>;

    async fn get_or_create_user_analysis(
        &self,
        user_id: &str,
    ) -> Result<UserAnalysis, Error>;

    /// Update user’s last_seen (and possibly username).
    async fn update_user_activity(
        &self,
        user_id: &str,
        new_username: Option<&str>,
    ) -> Result<(), Error>;
}

/// Struct stored in the in‐memory cache
#[derive(Debug, Clone)]
struct CachedUser {
    user: User,
    last_access: DateTime<Utc>,
}

pub struct DefaultUserManager {
    user_repo: UserRepository,
    identity_repo: PlatformIdentityRepository,
    analysis_repo: PostgresUserAnalysisRepository,

    /// Concurrency-safe map: (Platform, platform_user_id) -> CachedUser
    pub user_cache: DashMap<(Platform, String), CachedUser>,
}

/// Expire entries after 24 hours
const CACHE_MAX_AGE_SECS: i64 = 24 * 60 * 60; // 86400

impl DefaultUserManager {
    pub fn new(
        user_repo: UserRepository,
        identity_repo: PlatformIdentityRepository,
        analysis_repo: PostgresUserAnalysisRepository,
    ) -> Self {
        Self {
            user_repo,
            identity_repo,
            analysis_repo,
            // Start empty; DashMap is concurrency-safe
            user_cache: DashMap::new(),
        }
    }

    async fn insert_into_cache(
        &self,
        platform: Platform,
        platform_user_id: &str,
        user: &User
    ) {
        self.user_cache.insert(
            (platform, platform_user_id.to_string()),
            CachedUser {
                user: user.clone(),
                last_access: Utc::now(),
            }
        );
    }

    pub async fn invalidate_user_in_cache(&self, platform: Platform, platform_user_id: &str) {
        self.user_cache.remove(&(platform, platform_user_id.to_string()));
    }

    async fn prune_cache(&self) {
        let now = Utc::now();

        // Since DashMap doesn't have a built-in "retain" that locks each shard only once,
        // we gather keys that need removal, then remove them after.
        let mut to_remove = Vec::new();
        for entry in self.user_cache.iter() {
            let age = now.signed_duration_since(entry.value().last_access);
            if age >= Duration::seconds(CACHE_MAX_AGE_SECS) {
                to_remove.push(entry.key().clone());
            }
        }
        for key in to_remove {
            self.user_cache.remove(&key);
        }
    }

    pub async fn test_force_last_access(
        &self,
        platform: Platform,
        platform_user_id: &str,
        ago_hours: i64,
    ) -> bool {
        use chrono::Duration;

        let key = (platform, platform_user_id.to_string());
        if let Some(mut entry) = self.user_cache.get_mut(&key) {
            entry.last_access = Utc::now() - Duration::hours(ago_hours);
            true
        } else {
            false
        }
    }
}

#[async_trait]
impl UserManager for DefaultUserManager {
    async fn get_or_create_user(
        &self,
        platform: Platform,
        platform_user_id: &str,
        platform_username: Option<&str>,
    ) -> Result<User, Error> {
        // First prune old entries
        self.prune_cache().await;

        // Check the in-memory cache
        if let Some(mut entry) = self.user_cache.get_mut(&(platform.clone(), platform_user_id.to_string())) {
            // Found => update last_access & return
            entry.last_access = Utc::now();
            return Ok(entry.user.clone());
        }

        // If not in cache, check DB
        let existing_ident = self
            .identity_repo
            .get_by_platform(platform.clone(), platform_user_id)
            .await?;

        let user = if let Some(identity) = existing_ident {
            // fetch user from DB
            let db_user = self
                .user_repo
                .get(&identity.user_id)
                .await?
                .ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;

            // Store in cache
            self.insert_into_cache(platform.clone(), platform_user_id, &db_user).await;
            db_user
        } else {
            // not in DB => create
            let new_user_id = Uuid::new_v4().to_string();
            let now = Utc::now();
            let user = User {
                user_id: new_user_id.clone(),
                global_username: None,
                created_at: now,
                last_seen: now,
                is_active: true,
            };
            self.user_repo.create(&user).await?;

            let new_identity = PlatformIdentity {
                platform_identity_id: Uuid::new_v4().to_string(),
                user_id: new_user_id.clone(),
                platform: platform.clone(),
                platform_user_id: platform_user_id.to_string(),
                platform_username: platform_username.unwrap_or("unknown").to_string(),
                platform_display_name: None,
                platform_roles: vec![],
                platform_data: serde_json::json!({}),
                created_at: now,
                last_updated: now,
            };
            self.identity_repo.create(&new_identity).await?;

            // also create user_analysis row if needed
            let _analysis = self.get_or_create_user_analysis(&new_user_id).await?;

            // add to cache
            self.insert_into_cache(platform.clone(), platform_user_id, &user).await;
            user
        };

        Ok(user)
    }

    async fn get_or_create_user_analysis(
        &self,
        user_id: &str
    ) -> Result<UserAnalysis, Error> {
        if let Some(a) = self.analysis_repo.get_analysis(user_id).await? {
            return Ok(a);
        }
        let new_analysis = UserAnalysis::new(user_id);
        self.analysis_repo.create_analysis(&new_analysis).await?;
        Ok(new_analysis)
    }

    async fn update_user_activity(
        &self,
        user_id: &str,
        new_username: Option<&str>,
    ) -> Result<(), Error> {
        if let Some(mut user) = self.user_repo.get(user_id).await? {
            user.last_seen = Utc::now();
            if let Some(name) = new_username {
                user.global_username = Some(name.to_string());
            }
            self.user_repo.update(&user).await?;

            // Remove any matching entries from the cache so next call re-reads DB
            let mut keys_to_remove = Vec::new();
            for item in self.user_cache.iter() {
                if item.value().user.user_id == user_id {
                    keys_to_remove.push(item.key().clone());
                }
            }
            for k in keys_to_remove {
                self.user_cache.remove(&k);
            }
        }
        Ok(())
    }
}