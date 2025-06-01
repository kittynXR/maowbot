use crate::Error;
use async_trait::async_trait;
use chrono::{Utc};
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use maowbot_common::models::link_request::LinkRequest;
pub(crate) use maowbot_common::traits::repository_traits::LinkRequestsRepository;

#[derive(Clone)]
pub struct PostgresLinkRequestsRepository {
    pool: Pool<Postgres>,
}

impl PostgresLinkRequestsRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LinkRequestsRepository for PostgresLinkRequestsRepository {
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
            .bind(req.link_request_id)
            .bind(req.requesting_user_id)
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

    async fn get_link_request(&self, link_request_id: Uuid) -> Result<Option<LinkRequest>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                link_request_id,
                requesting_user_id,
                target_platform,
                target_platform_user_id,
                link_code,
                status,
                created_at,
                updated_at
            FROM link_requests
            WHERE link_request_id = $1
            "#,
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
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE link_requests
            SET requesting_user_id      = $1,
                target_platform         = $2,
                target_platform_user_id = $3,
                link_code               = $4,
                status                  = $5,
                updated_at              = $6
            WHERE link_request_id = $7
            "#,
        )
            .bind(req.requesting_user_id)
            .bind(&req.target_platform)
            .bind(&req.target_platform_user_id)
            .bind(&req.link_code)
            .bind(&req.status)
            .bind(now)
            .bind(req.link_request_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_link_request(&self, link_request_id: Uuid) -> Result<(), Error> {
        sqlx::query("DELETE FROM link_requests WHERE link_request_id = $1")
            .bind(link_request_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}