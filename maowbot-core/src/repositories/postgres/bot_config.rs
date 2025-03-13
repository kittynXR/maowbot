use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
pub(crate) use maowbot_common::traits::repository_traits::BotConfigRepository;
use crate::Error;

#[derive(Clone)]
pub struct PostgresBotConfigRepository {
    pool: Pool<Postgres>,
}

impl PostgresBotConfigRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BotConfigRepository for PostgresBotConfigRepository {
    async fn get_callback_port(&self) -> Result<Option<u16>, Error> {
        let row = sqlx::query(
            r#"
            SELECT config_value
            FROM bot_config
            WHERE config_key = 'callback_port'
            "#,
        )
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let val: String = r.try_get("config_value")?;
            if let Ok(parsed) = val.parse::<u16>() {
                Ok(Some(parsed))
            } else {
                // stored something non-numeric
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn set_callback_port(&self, port: u16) -> Result<(), Error> {
        self.set_value("callback_port", &port.to_string()).await
    }

    // -----------------------------------------------------------------------
    // "set_value" for old usage
    //
    // If you truly only want 1 row per `config_key`, you can choose a dummy
    // `config_value` like empty string ("") or the same as `config_key`.
    // But the ON CONFLICT must match (config_key, config_value).
    // -----------------------------------------------------------------------
    async fn set_value(&self, config_key: &str, config_value: &str) -> Result<(), Error> {
        // Use config_meta=NULL, composite key: (config_key, config_value)
        sqlx::query(
            r#"
            INSERT INTO bot_config (config_key, config_value, config_meta)
            VALUES ($1, $2, NULL)
            ON CONFLICT (config_key, config_value)
            DO UPDATE
               SET config_value = EXCLUDED.config_value,
                   config_meta  = NULL
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_value(&self, config_key: &str) -> Result<Option<String>, Error> {
        // If you only store a single row per config_key, you might also do:
        //  SELECT config_value FROM bot_config WHERE config_key=$1 LIMIT 1
        // and ignore multiple matches. This is what we do below.
        let row_opt = sqlx::query(
            r#"
            SELECT config_value
            FROM bot_config
            WHERE config_key = $1
            LIMIT 1
            "#,
        )
            .bind(config_key)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let val: String = row.try_get("config_value")?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    async fn list_all(&self) -> Result<Vec<(String, String)>, Error> {
        let rows = sqlx::query("SELECT config_key, config_value FROM bot_config")
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let k: String = row.try_get("config_key")?;
            let v: String = row.try_get("config_value")?;
            out.push((k, v));
        }
        Ok(out)
    }

    async fn delete_value(&self, config_key: &str) -> Result<(), Error> {
        // If we only want to remove the single row for `config_key`,
        // might do no filter on config_value. Or do a "LIMIT 1".
        // We'll remove *all* rows with that config_key.
        sqlx::query(
            r#"
            DELETE FROM bot_config
            WHERE config_key = $1
            "#,
        )
            .bind(config_key)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Extended usage: set_value_kv_meta and get_value_kv_meta
    // (multiple rows with different config_value per config_key).
    // ------------------------------------------------------------------------
    async fn set_value_kv_meta(
        &self,
        config_key: &str,
        config_value: &str,
        config_meta: Option<JsonValue>
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO bot_config (config_key, config_value, config_meta)
            VALUES ($1, $2, $3::jsonb)
            ON CONFLICT (config_key, config_value)
            DO UPDATE
               SET config_meta = EXCLUDED.config_meta
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .bind(config_meta)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_value_kv_meta(
        &self,
        config_key: &str,
        config_value: &str
    ) -> Result<Option<(String, Option<JsonValue>)>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT config_value, config_meta
            FROM bot_config
            WHERE config_key = $1
              AND config_value = $2
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let val: String = row.try_get("config_value")?;
            let meta_json: Option<JsonValue> = row.try_get("config_meta")?;
            Ok(Some((val, meta_json)))
        } else {
            Ok(None)
        }
    }

    async fn delete_value_kv(&self, config_key: &str, config_value: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM bot_config
            WHERE config_key = $1
              AND config_value = $2
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
