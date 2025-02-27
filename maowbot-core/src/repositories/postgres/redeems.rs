use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::Utc;
use crate::Error;
use crate::models::Redeem;

/// Repository for channel point rewards (redeems)
#[async_trait]
pub trait RedeemRepository: Send + Sync {
    async fn create_redeem(&self, rd: &Redeem) -> Result<(), Error>;
    async fn get_redeem_by_id(&self, redeem_id: Uuid) -> Result<Option<Redeem>, Error>;
    async fn get_redeem_by_reward_id(&self, platform: &str, reward_id: &str) -> Result<Option<Redeem>, Error>;
    async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error>;
    async fn update_redeem(&self, rd: &Redeem) -> Result<(), Error>;
    async fn delete_redeem(&self, redeem_id: Uuid) -> Result<(), Error>;
}

#[derive(Clone)]
pub struct PostgresRedeemRepository {
    pool: Pool<Postgres>,
}

impl PostgresRedeemRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RedeemRepository for PostgresRedeemRepository {
    async fn create_redeem(&self, rd: &Redeem) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO redeems (
                redeem_id, platform, reward_id, reward_name, cost,
                is_active, dynamic_pricing, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
            .bind(rd.redeem_id)
            .bind(&rd.platform)
            .bind(&rd.reward_id)
            .bind(&rd.reward_name)
            .bind(rd.cost)
            .bind(rd.is_active)
            .bind(rd.dynamic_pricing)
            .bind(rd.created_at)
            .bind(rd.updated_at)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_redeem_by_id(&self, redeem_id: Uuid) -> Result<Option<Redeem>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT redeem_id, platform, reward_id, reward_name, cost,
                   is_active, dynamic_pricing, created_at, updated_at
            FROM redeems
            WHERE redeem_id = $1
            "#,
        )
            .bind(redeem_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let rd = Redeem {
                redeem_id: row.try_get("redeem_id")?,
                platform: row.try_get("platform")?,
                reward_id: row.try_get("reward_id")?,
                reward_name: row.try_get("reward_name")?,
                cost: row.try_get("cost")?,
                is_active: row.try_get("is_active")?,
                dynamic_pricing: row.try_get("dynamic_pricing")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            Ok(Some(rd))
        } else {
            Ok(None)
        }
    }

    async fn get_redeem_by_reward_id(&self, platform: &str, reward_id: &str) -> Result<Option<Redeem>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT redeem_id, platform, reward_id, reward_name, cost,
                   is_active, dynamic_pricing, created_at, updated_at
            FROM redeems
            WHERE platform = $1
              AND reward_id = $2
            "#,
        )
            .bind(platform)
            .bind(reward_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let rd = Redeem {
                redeem_id: row.try_get("redeem_id")?,
                platform: row.try_get("platform")?,
                reward_id: row.try_get("reward_id")?,
                reward_name: row.try_get("reward_name")?,
                cost: row.try_get("cost")?,
                is_active: row.try_get("is_active")?,
                dynamic_pricing: row.try_get("dynamic_pricing")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            Ok(Some(rd))
        } else {
            Ok(None)
        }
    }

    async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT redeem_id, platform, reward_id, reward_name, cost,
                   is_active, dynamic_pricing, created_at, updated_at
            FROM redeems
            WHERE platform = $1
            ORDER BY reward_name ASC
            "#,
        )
            .bind(platform)
            .fetch_all(&self.pool)
            .await?;

        let mut result = Vec::new();
        for row in rows {
            let rd = Redeem {
                redeem_id: row.try_get("redeem_id")?,
                platform: row.try_get("platform")?,
                reward_id: row.try_get("reward_id")?,
                reward_name: row.try_get("reward_name")?,
                cost: row.try_get("cost")?,
                is_active: row.try_get("is_active")?,
                dynamic_pricing: row.try_get("dynamic_pricing")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            result.push(rd);
        }
        Ok(result)
    }

    async fn update_redeem(&self, rd: &Redeem) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE redeems
            SET reward_name = $1,
                cost = $2,
                is_active = $3,
                dynamic_pricing = $4,
                updated_at = $5
            WHERE redeem_id = $6
            "#,
        )
            .bind(&rd.reward_name)
            .bind(rd.cost)
            .bind(rd.is_active)
            .bind(rd.dynamic_pricing)
            .bind(rd.updated_at)
            .bind(rd.redeem_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_redeem(&self, redeem_id: Uuid) -> Result<(), Error> {
        sqlx::query("DELETE FROM redeems WHERE redeem_id = $1")
            .bind(redeem_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
