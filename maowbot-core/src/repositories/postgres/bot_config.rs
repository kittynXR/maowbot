use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use crate::Error;

#[async_trait]
pub trait BotConfigRepository: Send + Sync {
    async fn get_callback_port(&self) -> Result<Option<u16>, Error>;
    async fn set_callback_port(&self, port: u16) -> Result<(), Error>;
    async fn set_value(&self, config_key: &str, config_value: &str) -> Result<(), Error>;
    async fn get_value(&self, config_key: &str) -> Result<Option<String>, Error>;

    // NEW:
    async fn get_autostart(&self) -> Result<Option<String>, Error> {
        self.get_value("autostart").await
    }
    async fn set_autostart(&self, json_str: &str) -> Result<(), Error> {
        self.set_value("autostart", json_str).await
    }
    async fn list_all(&self) -> Result<Vec<(String, String)>, Error>;
}

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
            "#
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
        sqlx::query(
            r#"
            INSERT INTO bot_config (config_key, config_value)
            VALUES ($1, $2)
            ON CONFLICT (config_key) DO UPDATE
                SET config_value = EXCLUDED.config_value
            "#,
        )
            .bind(config_key)
            .bind(config_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_value(&self, config_key: &str) -> Result<Option<String>, Error> {
        let row = sqlx::query(
            r#"
            SELECT config_value
            FROM bot_config
            WHERE config_key = $1
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
}
