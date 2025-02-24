//! plugins/bot_api/user_api.rs
//!
//! Sub-trait with user account / profile methods (DB row in `users` table).

use crate::{Error, models::{User, PlatformIdentity, UserAnalysis}};
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

    /// Returns up to `limit` messages from chat_messages for the specified `user_id`,
    /// offset by `offset` for paging, optionally filtered by platform/channel, and text search.
    async fn get_user_chat_messages(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
        maybe_platform: Option<String>,
        maybe_channel: Option<String>,
        maybe_search: Option<String>,
    ) -> Result<Vec<crate::repositories::postgres::analytics::ChatMessage>, Error>;

    /// Appends (or sets) a moderator note in user_analysis for the given user_id.
    /// If user_analysis does not exist, create it. Then either append or override the existing note.
    async fn append_moderator_note(&self, user_id: Uuid, note_text: &str) -> Result<(), Error>;

    /// Returns all platform_identities for a given user.
    async fn get_platform_identities_for_user(&self, user_id: Uuid) -> Result<Vec<PlatformIdentity>, Error>;

    /// Returns the user_analysis row if present.
    async fn get_user_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error>;

    // -------------------------------
    // NEW MERGE METHOD
    // -------------------------------
    /// Merge user2’s data into user1, reassigning platform identities and chat messages.
    /// Optionally set a new global_username for user1. Then delete user2 from DB.
    async fn merge_users(
        &self,
        user1_id: Uuid,
        user2_id: Uuid,
        new_global_name: Option<&str>
    ) -> Result<(), Error>;
}