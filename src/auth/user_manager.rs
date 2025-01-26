// File: src/auth/user_manager.rs

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::Mutex;
use uuid::Uuid;
use async_trait::async_trait;

use crate::Error;
use crate::models::{User, Platform, PlatformIdentity};
use crate::models::user_analysis::UserAnalysis;
use crate::repositories::Repository;
use crate::repositories::sqlite::{
    user::UserRepository,
    platform_identity::PlatformIdentityRepository,
    user_analysis::{UserAnalysisRepository, SqliteUserAnalysisRepository},
};

/// Trait describing how our bot code “manages” user data, i.e. lookups and creation.
#[async_trait]
pub trait UserManager: Send + Sync {
    /// Looks up or creates a user record for `(platform, platform_user_id)`.
    /// If not found in memory nor DB, we create a new `User` + `PlatformIdentity`.
    async fn get_or_create_user(
        &self,
        platform: Platform,
        platform_user_id: &str,
        platform_username: Option<&str>,
    ) -> Result<User, Error>;

    /// Looks up or creates the user’s analysis/scoring row.
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
    user_id: String,
    pub last_access: DateTime<Utc>,  // for TTL expiry
}

/// Concrete implementation with in-memory caching & TTL
pub struct DefaultUserManager {
    user_repo: UserRepository,
    identity_repo: PlatformIdentityRepository,
    analysis_repo: SqliteUserAnalysisRepository,

    /// Maps (Platform, platform_user_id) -> CachedUser
    pub user_cache: Arc<Mutex<HashMap<(Platform, String), CachedUser>>>,
}

/// Expire entries after 24 hours
const CACHE_MAX_AGE_SECS: i64 = 24 * 60 * 60; // 86400

impl DefaultUserManager {
    pub fn new(
        user_repo: UserRepository,
        identity_repo: PlatformIdentityRepository,
        analysis_repo: SqliteUserAnalysisRepository,
    ) -> Self {
        Self {
            user_repo,
            identity_repo,
            analysis_repo,
            user_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Internal helper to remove any stale entries older than 24h
    async fn prune_cache(&self) {
        let now = Utc::now();
        let mut guard = self.user_cache.lock().await;
        guard.retain(|_, cached| {
            let age = now.signed_duration_since(cached.last_access);
            age < Duration::seconds(CACHE_MAX_AGE_SECS)
        });
    }

    /// [TEST-ONLY] Helper to forcibly set the last_access time
    /// for an existing cache entry (platform, platform_user_id).
    #[cfg(test)]
    pub async fn test_force_last_access(
        &self,
        platform: Platform,
        platform_user_id: &str,
        ago_hours: i64,
    ) -> bool {
        use chrono::{Utc, Duration};

        let mut lock = self.user_cache.lock().await;
        if let Some(entry) = lock.get_mut(&(platform, platform_user_id.to_string())) {
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
        // 1) Prune old entries
        self.prune_cache().await;

        // 2) Check the in-memory cache first
        {
            let mut cache_guard = self.user_cache.lock().await;
            if let Some(entry) = cache_guard.get_mut(&(platform.clone(), platform_user_id.to_string())) {
                // Check if it’s still fresh (prune_cache removed stale ones, so it should be good).
                // We can do a quick DB fetch if we need the entire `User` struct:
                let user_id = entry.user_id.clone();
                entry.last_access = Utc::now(); // update its last_access
                drop(cache_guard);

                if let Some(user) = self.user_repo.get(&user_id).await? {
                    return Ok(user);
                }
                // if DB lookup fails, we’ll fall through and do creation logic
            }
        }

        // 3) Not found in the cache => check DB by platform_identities
        let existing_ident = self
            .identity_repo
            .get_by_platform(platform.clone(), platform_user_id)
            .await?;

        if let Some(identity) = existing_ident {
            // Found an identity => fetch the user
            if let Some(db_user) = self.user_repo.get(&identity.user_id).await? {
                // Insert into the cache
                let mut cache_guard = self.user_cache.lock().await;
                cache_guard.insert(
                    (platform.clone(), platform_user_id.to_string()),
                    CachedUser {
                        user_id: db_user.user_id.clone(),
                        last_access: Utc::now(),
                    },
                );
                return Ok(db_user);
            }
        }

        // 4) Otherwise, create a new user + identity
        let new_user_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
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

        // Optionally create a default user_analysis row
        let _analysis = self.get_or_create_user_analysis(&new_user_id).await?;

        // 5) Insert into the cache
        let mut cache_guard = self.user_cache.lock().await;
        cache_guard.insert(
            (platform, platform_user_id.to_string()),
            CachedUser {
                user_id: new_user_id.clone(),
                last_access: Utc::now(),
            },
        );

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
            user.last_seen = Utc::now().naive_utc();
            if let Some(name) = new_username {
                user.global_username = Some(name.to_string());
            }
            self.user_repo.update(&user).await?;
        }
        Ok(())
    }
}

