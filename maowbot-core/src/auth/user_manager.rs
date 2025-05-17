use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use async_trait::async_trait;
use dashmap::DashMap;
use crate::Error;
use maowbot_common::models::user::{User};
use maowbot_common::models::platform::{ Platform, PlatformIdentity};
use maowbot_common::models::user_analysis::UserAnalysis;
pub(crate) use maowbot_common::traits::auth_traits::UserManager;

use crate::repositories::postgres::{
    user::UserRepository,
    platform_identity::PlatformIdentityRepository,
    user_analysis::{UserAnalysisRepository, PostgresUserAnalysisRepository},
};
use crate::repositories::postgres::platform_identity::PlatformIdentityRepo;
use crate::repositories::postgres::user::UserRepo;

#[derive(Debug, Clone)]
pub struct CachedUser {
    user: User,
    last_access: DateTime<Utc>,
}

pub struct DefaultUserManager {
    pub(crate) user_repo: Arc<UserRepository>,
    identity_repo: Arc<PlatformIdentityRepository>,
    analysis_repo: PostgresUserAnalysisRepository,
    pub user_cache: DashMap<(Platform, String), CachedUser>,
}

const CACHE_MAX_AGE_SECS: i64 = 24 * 3600;

impl DefaultUserManager {
    pub fn new(
        user_repo: Arc<UserRepository>,
        identity_repo: Arc<PlatformIdentityRepository>,
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
    ) -> Result<User, Error>
    {
        // ------------------------------------------------------------
        // 0.  Normalise IDs that come from the platform.
        //     – Twitch IRC occasionally gives us *either* the numeric
        //       user-id or the display-name, depending on message type.
        //     – Everything is forced to lowercase so we never create
        //       “Kittyn” vs “kittyn” duplicates.
        // ------------------------------------------------------------
        let lower_id = platform_user_id.to_ascii_lowercase();
        let mut lower_name = platform_username
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if lower_name.is_empty() {
            lower_name = lower_id.clone();
        }

        // ------------------------------------------------------------
        // 1.  Fast path:   in-memory cache hit?
        // ------------------------------------------------------------
        self.prune_cache().await;
        if let Some(mut cached) = self.user_cache.get_mut(&(platform.clone(), lower_id.clone())) {
            cached.last_access = Utc::now();
            return Ok(cached.user.clone());
        }

        // ------------------------------------------------------------
        // 2.  DB lookup on PRIMARY key (platform_user_id).
        // ------------------------------------------------------------
        if let Some(ident) = self
            .identity_repo
            .get_by_platform(platform.clone(), &lower_id)
            .await?
        {
            let db_user = self.user_repo
                .get(ident.user_id)
                .await?
                .ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;

            self.insert_into_cache(platform.clone(), &lower_id, &db_user).await;
            return Ok(db_user);
        }

        // ------------------------------------------------------------
        // 3.  Fallback DB lookup on DISPLAY-NAME.
        //     If we already created an identity with the display-name
        //     (because earlier we did *not* have the numeric id), reuse it
        //     **and** upgrade its `platform_user_id` so we never hit the
        //     fallback path again.
        // ------------------------------------------------------------
        if let Some(ident) = self
            .identity_repo
            .get_by_platform(platform.clone(), &lower_name)
            .await?
        {
            // If we have finally obtained the “real” id (numeric for Twitch),
            // patch the identity in-place.
            if ident.platform_user_id != lower_id {
                let mut patched = ident.clone();
                patched.platform_user_id = lower_id.clone();
                patched.last_updated = Utc::now();
                self.identity_repo.update(&patched).await?;
            }

            let db_user = self.user_repo
                .get(ident.user_id)
                .await?
                .ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;

            self.insert_into_cache(platform.clone(), &lower_id, &db_user).await;
            return Ok(db_user);
        }

        // ------------------------------------------------------------
        // 4.  Nothing found → create brand-new user *and* identity.
        // ------------------------------------------------------------
        let now = Utc::now();
        let new_user_id = Uuid::new_v4();
        let mut user = User {
            user_id: new_user_id,
            global_username: if lower_name.is_empty() { None } else { Some(lower_name.clone()) },
            created_at: now,
            last_seen: now,
            is_active: true,
        };
        self.user_repo.create(&user).await?;

        let ident = PlatformIdentity {
            platform_identity_id: Uuid::new_v4(),
            user_id:           new_user_id,
            platform:          platform.clone(),
            platform_user_id:  lower_id.clone(),
            platform_username: lower_name.clone(),
            platform_display_name: None,
            platform_roles:    vec![],
            platform_data:     serde_json::json!({}),
            created_at:        now,
            last_updated:      now,
        };
        self.identity_repo.create(&ident).await?;

        // pre-create empty analysis row
        let _ = self.get_or_create_user_analysis(new_user_id).await?;

        self.insert_into_cache(platform.clone(), &lower_id, &user).await;
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
