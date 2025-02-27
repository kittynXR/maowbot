use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::Utc;
use crate::Error;
use crate::models::RedeemUsage;

/// Repository for redeem usage logs
#[async_trait]
pub trait RedeemUsageRepository: Send + Sync {
    async fn insert_usage(&self, usage: &RedeemUsage) -> Result<(), Error>;
    async fn list_usage_for_redeem(&self, redeem_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
    async fn list_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error>;
}

#[derive(Clone)]
pub struct PostgresRedeemUsageRepository {
    pool: Pool<Postgres>,
}

impl PostgresRedeemUsageRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RedeemUsageRepository for PostgresRedeemUsageRepository {
    async fn insert_usage(&self, usage: &RedeemUsage) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO redeem_usage (
                usage_id, redeem_id, user_id, used_at,
                channel, usage_data
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
            .bind(usage.usage_id)
            .bind(usage.redeem_id)
            .bind(usage.user_id)
            .bind(usage.used_at)
            .bind(&usage.channel)
            .bind(&usage.usage_data)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn list_usage_for_redeem(&self, redeem_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT usage_id, redeem_id, user_id, used_at,
                   channel, usage_data
            FROM redeem_usage
            WHERE redeem_id = $1
            ORDER BY used_at DESC
            LIMIT $2
            "#,
        )
            .bind(redeem_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut result = Vec::new();
        for row in rows {
            let ru = RedeemUsage {
                usage_id: row.try_get("usage_id")?,
                redeem_id: row.try_get("redeem_id")?,
                user_id: row.try_get("user_id")?,
                used_at: row.try_get("used_at")?,
                channel: row.try_get("channel")?,
                usage_data: row.try_get("usage_data")?,
            };
            result.push(ru);
        }
        Ok(result)
    }

    async fn list_usage_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<RedeemUsage>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT usage_id, redeem_id, user_id, used_at,
                   channel, usage_data
            FROM redeem_usage
            WHERE user_id = $1
            ORDER BY used_at DESC
            LIMIT $2
            "#,
        )
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut result = Vec::new();
        for row in rows {
            let ru = RedeemUsage {
                usage_id: row.try_get("usage_id")?,
                redeem_id: row.try_get("redeem_id")?,
                user_id: row.try_get("user_id")?,
                used_at: row.try_get("used_at")?,
                channel: row.try_get("channel")?,
                usage_data: row.try_get("usage_data")?,
            };
            result.push(ru);
        }
        Ok(result)
    }
}
