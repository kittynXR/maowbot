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
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn set_callback_port(&self, port: u16) -> Result<(), Error> {
        let port_str = port.to_string();
        self.set_value("callback_port", &port_str).await
    }

    async fn set_value(&self, config_key: &str, config_value: &str) -> Result<(), Error> {
        // This usage treats config_key as a unique row. We do NOT store multiple
        // config_values under the same key in this method. We store the “old style”.
        // So we just do upsert on config_key alone, ignoring config_meta for now.
        //
        // Because the table is now using (config_key, config_value) as a composite PK,
        // we do an upsert with config_value also in the primary key. That means
        // we *overwrite* the old row’s config_value if it existed. In effect,
        // we set config_value = <some string> and config_meta = NULL.
        //
        // If you want to store multiple values under the same key, use set_value_kv_meta.
        //
        sqlx::query(
            r#"
            INSERT INTO bot_config (config_key, config_value, config_meta)
            VALUES ($1, $2, NULL)
            ON CONFLICT (config_key)
            DO UPDATE SET
               config_value = EXCLUDED.config_value,
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
        // We look for any row that has this config_key.
        // Because we might have multiple rows with the same config_key but different
        // config_value in the new schema, we just pick the “first” or some row.
        // For backward compatibility, we pick the row with matching config_key
        // ignoring the config_value. If multiple exist, we arbitrarily return one.
        let row = sqlx::query(
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

        if let Some(r) = row {
            Ok(Some(r.try_get("config_value")?))
        } else {
            Ok(None)
        }
    }

    async fn list_all(&self) -> Result<Vec<(String, String)>, Error> {
        let rows = sqlx::query(r#"SELECT config_key, config_value FROM bot_config"#)
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
    // New methods for composite usage: (config_key, config_value)
    // with config_meta JSONB
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
            VALUES ($1, $2, $3)
            ON CONFLICT (config_key, config_value)
            DO UPDATE
               SET config_meta = EXCLUDED.config_meta
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .bind(config_meta.map(|j| j.to_string()))
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
            WHERE config_key = $1 AND config_value = $2
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let val: String = row.try_get("config_value")?;
            let meta_str: Option<String> = row.try_get("config_meta")?;
            let meta_json: Option<JsonValue> = if let Some(s) = meta_str {
                serde_json::from_str(&s).ok()
            } else {
                None
            };
            Ok(Some((val, meta_json)))
        } else {
            Ok(None)
        }
    }

    async fn delete_value_kv(&self, config_key: &str, config_value: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM bot_config
            WHERE config_key = $1 AND config_value = $2
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
