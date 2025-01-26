use sqlx::{Pool, Sqlite, Row};
use crate::Error;
use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

/// Reflects one row in the `link_requests` table
#[derive(Debug, Clone)]
pub struct LinkRequest {
    pub link_request_id: String,
    pub requesting_user_id: String,
    pub target_platform: Option<String>,
    pub target_platform_user_id: Option<String>,
    pub link_code: Option<String>,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl LinkRequest {
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
        sqlx::query(
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
            "#
        )
            .bind(&req.link_request_id)
            .bind(&req.requesting_user_id)
            .bind(&req.target_platform)
            .bind(&req.target_platform_user_id)
            .bind(&req.link_code)
            .bind(&req.status)
            .bind(req.created_at)
            .bind(req.updated_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_link_request(&self, link_request_id: &str) -> Result<Option<LinkRequest>, Error> {
        let row = sqlx::query(
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
            "#
        )
            .bind(link_request_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(LinkRequest {
                link_request_id: r.try_get("link_request_id")?,
                requesting_user_id: r.try_get("requesting_user_id")?,
                target_platform: r.try_get("target_platform")?,
                target_platform_user_id: r.try_get("target_platform_user_id")?,
                link_code: r.try_get("link_code")?,
                status: r.try_get("status")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_link_request(&self, req: &LinkRequest) -> Result<(), Error> {
        let now = Utc::now().naive_utc();

        sqlx::query(
            r#"
            UPDATE link_requests
            SET requesting_user_id = ?,
                target_platform = ?,
                target_platform_user_id = ?,
                link_code = ?,
                status = ?,
                updated_at = ?
            WHERE link_request_id = ?
            "#
        )
            .bind(&req.requesting_user_id)
            .bind(&req.target_platform)
            .bind(&req.target_platform_user_id)
            .bind(&req.link_code)
            .bind(&req.status)
            .bind(now)
            .bind(&req.link_request_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_link_request(&self, link_request_id: &str) -> Result<(), Error> {
        sqlx::query(
            "DELETE FROM link_requests WHERE link_request_id = ?"
        )
            .bind(link_request_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
