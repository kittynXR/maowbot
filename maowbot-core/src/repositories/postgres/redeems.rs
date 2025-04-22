// File: maowbot-core/src/repositories/postgres/redeems.rs

use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::Utc;
use maowbot_common::error::Error;
use maowbot_common::models::redeem::{Redeem};
use maowbot_common::traits::repository_traits::RedeemRepository;

pub struct PostgresRedeemRepository {
    pub pool: Pool<Postgres>,
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
                redeem_id,
                platform,
                reward_id,
                reward_name,
                cost,
                is_active,
                dynamic_pricing,
                active_offline,
                is_managed,
                plugin_name,
                command_name,
                created_at,
                updated_at,
                active_credential_id,
                is_user_input_required
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
            "#,
        )
            .bind(rd.redeem_id)
            .bind(&rd.platform)
            .bind(&rd.reward_id)
            .bind(&rd.reward_name)
            .bind(rd.cost)
            .bind(rd.is_active)
            .bind(rd.dynamic_pricing)
            .bind(rd.active_offline)
            .bind(rd.is_managed)
            .bind(&rd.plugin_name)
            .bind(&rd.command_name)
            .bind(rd.created_at)
            .bind(rd.updated_at)
            .bind(rd.active_credential_id)
            .bind(rd.is_user_input_required)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_redeem_by_id(&self, redeem_id: Uuid) -> Result<Option<Redeem>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                redeem_id,
                platform,
                reward_id,
                reward_name,
                cost,
                is_active,
                dynamic_pricing,
                active_offline,
                is_managed,
                plugin_name,
                command_name,
                created_at,
                updated_at,
                active_credential_id,
                is_user_input_required
            FROM redeems
            WHERE redeem_id = $1
            "#,
        )
            .bind(redeem_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let rd = Redeem {
                redeem_id: r.try_get("redeem_id")?,
                platform: r.try_get("platform")?,
                reward_id: r.try_get("reward_id")?,
                reward_name: r.try_get("reward_name")?,
                cost: r.try_get("cost")?,
                is_active: r.try_get("is_active")?,
                dynamic_pricing: r.try_get("dynamic_pricing")?,
                active_offline: r.try_get("active_offline")?,
                is_managed: r.try_get("is_managed")?,
                plugin_name: r.try_get("plugin_name")?,
                command_name: r.try_get("command_name")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                active_credential_id: r.try_get("active_credential_id")?,
                is_user_input_required: r.try_get("is_user_input_required").unwrap_or(false),
            };
            Ok(Some(rd))
        } else {
            Ok(None)
        }
    }

    async fn get_redeem_by_reward_id(&self, platform: &str, reward_id: &str) -> Result<Option<Redeem>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                redeem_id,
                platform,
                reward_id,
                reward_name,
                cost,
                is_active,
                dynamic_pricing,
                active_offline,
                is_managed,
                plugin_name,
                command_name,
                created_at,
                updated_at,
                active_credential_id,
                is_user_input_required
            FROM redeems
            WHERE LOWER(platform) = LOWER($1)
              AND LOWER(reward_id) = LOWER($2)
            "#,
        )
            .bind(platform)
            .bind(reward_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let rd = Redeem {
                redeem_id: r.try_get("redeem_id")?,
                platform: r.try_get("platform")?,
                reward_id: r.try_get("reward_id")?,
                reward_name: r.try_get("reward_name")?,
                cost: r.try_get("cost")?,
                is_active: r.try_get("is_active")?,
                dynamic_pricing: r.try_get("dynamic_pricing")?,
                active_offline: r.try_get("active_offline")?,
                is_managed: r.try_get("is_managed")?,
                plugin_name: r.try_get("plugin_name")?,
                command_name: r.try_get("command_name")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                active_credential_id: r.try_get("active_credential_id")?,
                is_user_input_required: r.try_get("is_user_input_required").unwrap_or(false),
            };
            Ok(Some(rd))
        } else {
            Ok(None)
        }
    }

    async fn list_redeems(&self, platform: &str) -> Result<Vec<Redeem>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                redeem_id,
                platform,
                reward_id,
                reward_name,
                cost,
                is_active,
                dynamic_pricing,
                active_offline,
                is_managed,
                plugin_name,
                command_name,
                created_at,
                updated_at,
                active_credential_id,
                is_user_input_required
            FROM redeems
            WHERE LOWER(platform) = LOWER($1)
            ORDER BY reward_name ASC
            "#,
        )
            .bind(platform)
            .fetch_all(&self.pool)
            .await?;

        let mut list = Vec::new();
        for r in rows {
            let rd = Redeem {
                redeem_id: r.try_get("redeem_id")?,
                platform: r.try_get("platform")?,
                reward_id: r.try_get("reward_id")?,
                reward_name: r.try_get("reward_name")?,
                cost: r.try_get("cost")?,
                is_active: r.try_get("is_active")?,
                dynamic_pricing: r.try_get("dynamic_pricing")?,
                active_offline: r.try_get("active_offline")?,
                is_managed: r.try_get("is_managed")?,
                plugin_name: r.try_get("plugin_name")?,
                command_name: r.try_get("command_name")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
                active_credential_id: r.try_get("active_credential_id")?,
                is_user_input_required: r.try_get("is_user_input_required").unwrap_or(false),
            };
            list.push(rd);
        }
        Ok(list)
    }

    async fn update_redeem(&self, rd: &Redeem) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE redeems
            SET
              platform = $1,
              reward_id = $2,
              reward_name = $3,
              cost = $4,
              is_active = $5,
              dynamic_pricing = $6,
              active_offline = $7,
              is_managed = $8,
              plugin_name = $9,
              command_name = $10,
              updated_at = $11,
              active_credential_id = $12,
              is_user_input_required = $13
            WHERE redeem_id = $14
            "#,
        )
            .bind(&rd.platform)
            .bind(&rd.reward_id)
            .bind(&rd.reward_name)
            .bind(rd.cost)
            .bind(rd.is_active)
            .bind(rd.dynamic_pricing)
            .bind(rd.active_offline)
            .bind(rd.is_managed)
            .bind(&rd.plugin_name)
            .bind(&rd.command_name)
            .bind(rd.updated_at)
            .bind(rd.active_credential_id)
            .bind(rd.is_user_input_required)
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
