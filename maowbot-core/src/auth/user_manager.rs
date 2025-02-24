use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use async_trait::async_trait;
use dashmap::DashMap;

use crate::Error;
use crate::models::{User, Platform, PlatformIdentity};
use crate::models::user_analysis::UserAnalysis;
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

    /// Now takes a Uuid instead of &str:
    async fn get_or_create_user_analysis(&self, user_id: Uuid) -> Result<UserAnalysis, Error>;

    async fn update_user_activity(&self, user_id: &str, new_username: Option<&str>)
                                  -> Result<(), Error>;
}

#[derive(Debug, Clone)]
struct CachedUser {
    user: User,
    last_access: DateTime<Utc>,
}

pub struct DefaultUserManager {
    pub(crate) user_repo: Arc<UserRepository>,
    identity_repo: PlatformIdentityRepository,
    analysis_repo: PostgresUserAnalysisRepository,
    pub user_cache: DashMap<(Platform, String), CachedUser>,
}

const CACHE_MAX_AGE_SECS: i64 = 24 * 3600;

impl DefaultUserManager {
    pub fn new(
        user_repo: Arc<UserRepository>,
        identity_repo: PlatformIdentityRepository,
        analysis_repo: PostgresUserAnalysisRepository,
    ) -> Self {
        Self {
            user_repo,
            identity_repo,
            analysis_repo,
            user_cache: DashMap::new(),
        }
    }

    async fn insert_into_cache(
        &self,
        platform: Platform,
        platform_user_id: &str,
        user: &User,
    ) {
        self.user_cache.insert(
            (platform, platform_user_id.to_string()),
            CachedUser {
                user: user.clone(),
                last_access: Utc::now(),
            },
        );
    }

    pub async fn invalidate_user_in_cache(&self, platform: Platform, platform_user_id: &str) {
        self.user_cache.remove(&(platform, platform_user_id.to_string()));
    }

    async fn prune_cache(&self) {
        let now = Utc::now();
        let mut to_remove = Vec::new();
        for entry in self.user_cache.iter() {
            let age = now.signed_duration_since(entry.value().last_access);
            if age.num_seconds() >= CACHE_MAX_AGE_SECS {
                to_remove.push(entry.key().clone());
            }
        }
        for key in to_remove {
            self.user_cache.remove(&key);
        }
    }

    /// Test helper
    pub async fn test_force_last_access(
        &self,
        platform: Platform,
        platform_user_id: &str,
        hours_ago: i64,
    ) -> bool {
        let key = (platform, platform_user_id.to_string());
        if let Some(mut entry) = self.user_cache.get_mut(&key) {
            entry.last_access = Utc::now() - chrono::Duration::hours(hours_ago);
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
        // 1) prune old cache entries
        self.prune_cache().await;

        // 2) unify the platform_user_id by forcing to lowercase (or do a case-insensitive DB search).
        //    We'll store it in DB as all-lowercase to avoid duplicates like "Kittyn" vs "kittyn".
        let lower_id = platform_user_id.to_lowercase();

        // 3) check in-memory cache
        if let Some(mut entry) = self.user_cache.get_mut(&(platform.clone(), lower_id.clone())) {
            entry.last_access = Utc::now();
            return Ok(entry.user.clone());
        }

        // 4) check DB for a matching identity
        let existing_ident = self
            .identity_repo
            .get_by_platform(platform.clone(), &lower_id)
            .await?;

        let user = if let Some(ident) = existing_ident {
            let db_user = self.user_repo
                .get(ident.user_id)
                .await?
                .ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;

            // Cache it
            self.insert_into_cache(platform.clone(), &lower_id, &db_user).await;
            db_user
        } else {
            // create a new user
            let new_user_id = Uuid::new_v4();
            let now = Utc::now();
            let mut user = User {
                user_id: new_user_id,
                global_username: None,
                created_at: now,
                last_seen: now,
                is_active: true,
            };
            // If a platform_username is provided, set the global_username at creation
            if let Some(name) = platform_username {
                if !name.trim().is_empty() {
                    user.global_username = Some(name.trim().to_string());
                }
            }

            self.user_repo.create(&user).await?;

            // new identity
            let new_identity_id = Uuid::new_v4();
            let identity = PlatformIdentity {
                platform_identity_id: new_identity_id,
                user_id: new_user_id,
                platform: platform.clone(),
                // store in DB as all-lowercase
                platform_user_id: lower_id.clone(),
                // use the original username param if we want
                platform_username: platform_username.unwrap_or("unknown").to_string(),
                platform_display_name: None,
                platform_roles: vec![],
                platform_data: serde_json::json!({}),
                created_at: now,
                last_updated: now,
            };
            self.identity_repo.create(&identity).await?;

            // also create analysis row
            let _analysis = self.get_or_create_user_analysis(new_user_id).await?;

            // store in cache
            self.insert_into_cache(platform.clone(), &lower_id, &user).await;
            user
        };

        Ok(user)
    }

    async fn get_or_create_user_analysis(
        &self,
        user_id: Uuid,
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
        let parsed_id = Uuid::parse_str(user_id)
            .map_err(|e| Error::Auth(format!("Cannot parse user_id as UUID: {e}")))?;

        if let Some(mut user) = self.user_repo.get(parsed_id).await? {
            user.last_seen = Utc::now();
            if let Some(name) = new_username {
                if !name.trim().is_empty() {
                    user.global_username = Some(name.trim().to_string());
                }
            }
            self.user_repo.update(&user).await?;

            // remove from cache in case we want to refresh
            let mut keys_to_remove = Vec::new();
            for entry in self.user_cache.iter() {
                if entry.value().user.user_id == parsed_id {
                    keys_to_remove.push(entry.key().clone());
                }
            }
            for k in keys_to_remove {
                self.user_cache.remove(&k);
            }
        }
        Ok(())
    }
}
