//! plugins/bot_api/user_api.rs
//!
//! Sub-trait with user account / profile methods (DB row in `users` table).

use crate::{Error, models::User};
use async_trait::async_trait;
use uuid::Uuid;

/// Methods for basic user management (create, remove, find, etc.).
#[async_trait]
pub trait UserApi: Send + Sync {
    /// Create a new user row in the DB with the given UUID and display name.
    async fn create_user(&self, new_user_id: Uuid, display_name: &str) -> Result<(), Error>;

    /// Remove a user by UUID.
    async fn remove_user(&self, user_id: Uuid) -> Result<(), Error>;

    /// Fetch a user by UUID, returning None if not found.
    async fn get_user(&self, user_id: Uuid) -> Result<Option<User>, Error>;

    /// Update the user’s `is_active` boolean in the DB.
    async fn update_user_active(&self, user_id: Uuid, is_active: bool) -> Result<(), Error>;

    /// Search users by some textual query. Could be partial string match on username or user_id.
    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error>;

    /// Look up a single user by name, returning an error if not found or if multiple matches.
    async fn find_user_by_name(&self, name: &str) -> Result<User, Error>;
}