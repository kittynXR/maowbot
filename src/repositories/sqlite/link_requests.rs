use sqlx::{Pool, Sqlite};
use crate::Error;
use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

/// Reflects one row in the `link_requests` table
#[derive(Debug, Clone)]
pub struct LinkRequest {
    pub link_request_id: String,
    pub requesting_user_id: String,       // references users(user_id)
    pub target_platform: Option<String>,  // e.g. "twitch"
    pub target_platform_user_id: Option<String>,
    pub link_code: Option<String>,
    pub status: String,                   // "pending", "approved", "denied", etc
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl LinkRequest {
    /// Helper: create a new pending link request
    pub fn new(
        requesting_user_id: &str,
        target_platform: Option<&str>,
        target_platform_user_id: Option<&str>,
        link_code: Option<&str>,
    ) -> Self {
        Self {
            link_request_id: Uuid::new_v4().to_string(),
            requesting_user_id: requesting_user_id.to_string(),
            target_platform: target_platform.map(|s| s.to_string()),
            target_platform_user_id: target_platform_user_id.map(|s| s.to_string()),
            link_code: link_code.map(|s| s.to_string()),
            status: "pending".to_string(),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        }
    }
}

/// Minimal trait for LinkRequests (you can fold this into `Repository` if you prefer)
#[async_trait]
pub trait LinkRequestsRepository {
    async fn create_link_request(&self, req: &LinkRequest) -> Result<(), Error>;
    async fn get_link_request(&self, link_request_id: &str) -> Result<Option<LinkRequest>, Error>;
    async fn update_link_request(&self, req: &LinkRequest) -> Result<(), Error>;
    async fn delete_link_request(&self, link_request_id: &str) -> Result<(), Error>;
}

/// Concrete implementation with SQLite
#[derive(Clone)]
pub struct SqliteLinkRequestsRepository {
    pool: Pool<Sqlite>,
}

impl SqliteLinkRequestsRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LinkRequestsRepository for SqliteLinkRequestsRepository {
    async fn create_link_request(&self, req: &LinkRequest) -> Result<(), Error> {
        sqlx::query!(
            r#"
            INSERT INTO link_requests (
                link_request_id,
                requesting_user_id,
                target_platform,
                target_platform_user_id,
                link_code,
                status,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            req.link_request_id,
            req.requesting_user_id,
            req.target_platform,
            req.target_platform_user_id,
            req.link_code,
            req.status,
            req.created_at,
            req.updated_at
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_link_request(&self, link_request_id: &str) -> Result<Option<LinkRequest>, Error> {
        let row = sqlx::query!(
            r#"
            SELECT link_request_id,
                   requesting_user_id,
                   target_platform,
                   target_platform_user_id,
                   link_code,
                   status,
                   created_at,
                   updated_at
            FROM link_requests
            WHERE link_request_id = ?
            "#,
            link_request_id
        )
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(LinkRequest {
                link_request_id: r.link_request_id,
                requesting_user_id: r.requesting_user_id,
                target_platform: r.target_platform,
                target_platform_user_id: r.target_platform_user_id,
                link_code: r.link_code,
                status: r.status,
                created_at: r.created_at,
                updated_at: r.updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_link_request(&self, req: &LinkRequest) -> Result<(), Error> {
        // fix: create a local variable for updated_at so query! doesn't borrow a temp
        let now = Utc::now().naive_utc();

        sqlx::query!(
            r#"
            UPDATE link_requests
            SET requesting_user_id = ?,
                target_platform = ?,
                target_platform_user_id = ?,
                link_code = ?,
                status = ?,
                updated_at = ?
            WHERE link_request_id = ?
            "#,
            req.requesting_user_id,
            req.target_platform,
            req.target_platform_user_id,
            req.link_code,
            req.status,
            now, // we pass the local variable here
            req.link_request_id
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_link_request(&self, link_request_id: &str) -> Result<(), Error> {
        sqlx::query!(
            "DELETE FROM link_requests WHERE link_request_id = ?",
            link_request_id
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
