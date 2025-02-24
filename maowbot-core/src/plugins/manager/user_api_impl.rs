//! plugins/manager/user_api_impl.rs
//!
//! Implements UserApi for PluginManager (create_user, remove_user, etc.).

use std::sync::Arc;
use uuid::Uuid;
use async_trait::async_trait;
use crate::Error;
use crate::models::{User, PlatformIdentity, UserAnalysis};
use crate::plugins::bot_api::user_api::UserApi;
use crate::plugins::manager::core::PluginManager;
use crate::repositories::postgres::user::UserRepo;
use crate::repositories::postgres::platform_identity::PlatformIdentityRepo;
use crate::repositories::postgres::analytics::AnalyticsRepo;
use crate::repositories::postgres::user_analysis::UserAnalysisRepository;

#[async_trait]
impl UserApi for PluginManager {
    async fn create_user(&self, new_user_id: Uuid, display_name: &str) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        let user = crate::models::User {
            user_id: new_user_id,
            global_username: Some(display_name.to_string()),
            created_at: chrono::Utc::now(),
            last_seen: chrono::Utc::now(),
            is_active: true,
        };
        user_repo.create(&user).await?;
        Ok(())
    }

    async fn remove_user(&self, user_id: Uuid) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        user_repo.delete(user_id).await?;
        Ok(())
    }

    async fn get_user(&self, user_id: Uuid) -> Result<Option<User>, Error> {
        let user_repo = self.user_repo.clone();
        Ok(user_repo.get(user_id).await?)
    }

    async fn update_user_active(&self, user_id: Uuid, is_active: bool) -> Result<(), Error> {
        let user_repo = self.user_repo.clone();
        if let Some(mut u) = user_repo.get(user_id).await? {
            u.is_active = is_active;
            u.last_seen = chrono::Utc::now();
            user_repo.update(&u).await?;
        }
        Ok(())
    }

    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error> {
        let user_repo = self.user_repo.clone();
        let all_users = user_repo.list_all().await?;
        let filtered = all_users
            .into_iter()
            .filter(|u| {
                let user_id_str = u.user_id.to_string();
                let uname = u.global_username.as_deref().unwrap_or("");
                user_id_str.contains(&query) || uname.to_lowercase().contains(&query.to_lowercase())
            })
            .collect();
        Ok(filtered)
    }

    async fn find_user_by_name(&self, name: &str) -> Result<User, Error> {
        let all = self.search_users(name).await?;
        let matches: Vec<User> = all.into_iter()
            .filter(|u| {
                let uname = u.global_username.as_deref().unwrap_or("").to_lowercase();
                uname == name.to_lowercase()
            })
            .collect();
        if matches.is_empty() {
            Err(Error::Auth(format!("No user with name='{name}'")))
        } else if matches.len() > 1 {
            Err(Error::Auth(format!("Multiple matches for '{name}'")))
        } else {
            Ok(matches[0].clone())
        }
    }

    // NEW:
    async fn get_user_chat_messages(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        maybe_platform: Option<String>,
        maybe_channel: Option<String>,
        maybe_search: Option<String>,
    ) -> Result<Vec<crate::repositories::postgres::analytics::ChatMessage>, Error> {
        let analytics_repo = self.analytics_repo.clone();
        let messages = analytics_repo.get_messages_for_user(
            user_id,
            limit,
            offset,
            maybe_platform.as_deref(),
            maybe_channel.as_deref(),
            maybe_search.as_deref(),
        ).await?;
        Ok(messages)
    }

    async fn append_moderator_note(&self, user_id: Uuid, note_text: &str) -> Result<(), Error> {
        let analysis_repo = self.user_analysis_repo.clone();

        // check if user_analysis exists
        let existing_opt = analysis_repo.get_analysis(user_id).await?;
        if let Some(mut existing) = existing_opt {
            // if there's existing moderator_notes, append
            if let Some(old_notes) = &existing.moderator_notes {
                let new_text = format!("{}\n{}", old_notes, note_text);
                existing.moderator_notes = Some(new_text);
            } else {
                existing.moderator_notes = Some(note_text.to_string());
            }
            existing.updated_at = chrono::Utc::now();
            analysis_repo.update_analysis(&existing).await?;
        } else {
            // create a new user_analysis row
            let mut new_ua = crate::models::UserAnalysis::new(user_id);
            new_ua.moderator_notes = Some(note_text.to_string());
            analysis_repo.create_analysis(&new_ua).await?;
        }

        Ok(())
    }

    async fn get_platform_identities_for_user(&self, user_id: Uuid) -> Result<Vec<PlatformIdentity>, Error> {
        let pi_repo = self.platform_identity_repo.clone();
        let results = pi_repo.get_all_for_user(user_id).await?;
        Ok(results)
    }

    async fn get_user_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error> {
        let ua_repo = self.user_analysis_repo.clone();
        let ua = ua_repo.get_analysis(user_id).await?;
        Ok(ua)
    }
}
