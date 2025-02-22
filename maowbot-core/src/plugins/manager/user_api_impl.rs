//! plugins/manager/user_api_impl.rs
//!
//! Implements UserApi for PluginManager (create_user, remove_user, etc.).

use std::sync::Arc;
use uuid::Uuid;
use async_trait::async_trait;
use crate::Error;
use crate::models::User;
use crate::plugins::bot_api::user_api::UserApi;
use crate::plugins::manager::core::PluginManager;
use crate::repositories::postgres::user::UserRepo;

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
                user_id_str.contains(query)
                    || u.global_username.as_deref().unwrap_or("").contains(query)
            })
            .collect();
        Ok(filtered)
    }

    async fn find_user_by_name(&self, name: &str) -> Result<User, Error> {
        let all = self.search_users(name).await?;
        if all.is_empty() {
            Err(Error::Auth(format!("No user with name='{name}'")))
        } else if all.len() > 1 {
            Err(Error::Auth(format!("Multiple matches for '{name}'")))
        } else {
            Ok(all[0].clone())
        }
    }
}