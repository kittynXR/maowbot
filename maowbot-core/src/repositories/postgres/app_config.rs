use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use crate::Error;

#[async_trait]
pub trait AppConfigRepository: Send + Sync {
    /// Returns the port stored under key="callback_port", or None if not set.
    async fn get_callback_port(&self) -> Result<Option<u16>, Error>;

    /// Sets the callback port in the app_config table.
    async fn set_callback_port(&self, port: u16) -> Result<(), Error>;

    /// Generic setter for any string state_value, keyed by state_key.
    async fn set_value(&self, state_key: &str, state_value: &str) -> Result<(), Error>;

    /// Generic getter for any string state_value, keyed by state_key.
    async fn get_value(&self, state_key: &str) -> Result<Option<String>, Error>;
}

#[derive(Clone)]
pub struct PostgresAppConfigRepository {
    pool: Pool<Postgres>,
}

impl PostgresAppConfigRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AppConfigRepository for PostgresAppConfigRepository {
    async fn get_callback_port(&self) -> Result<Option<u16>, Error> {
        let row = sqlx::query(
            r#"
            SELECT state_value
            FROM app_config
            WHERE state_key = 'callback_port'
            "#
        )
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let val: String = r.try_get("state_value")?;
            // Attempt parse u16
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

    async fn set_value(&self, state_key: &str, state_value: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO app_config (state_key, state_value)
            VALUES ($1, $2)
            ON CONFLICT (state_key) DO UPDATE
                SET state_value = EXCLUDED.state_value
            "#
        )
            .bind(state_key)
            .bind(state_value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_value(&self, state_key: &str) -> Result<Option<String>, Error> {
        let row = sqlx::query(
            r#"
            SELECT state_value
            FROM app_config
            WHERE state_key = $1
            "#,
        )
            .bind(state_key)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(r.try_get("state_value")?))
        } else {
            Ok(None)
        }
    }
}