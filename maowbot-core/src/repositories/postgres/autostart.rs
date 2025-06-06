use sqlx::{Pool, Postgres};
use async_trait::async_trait;
use crate::Error;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AutostartEntry {
    pub id: i32,
    pub platform: String,
    pub account_name: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait AutostartRepository: Send + Sync {
    /// Get all enabled autostart entries
    async fn get_enabled_entries(&self) -> Result<Vec<AutostartEntry>, Error>;
    
    /// Get all autostart entries (enabled and disabled)
    async fn get_all_entries(&self) -> Result<Vec<AutostartEntry>, Error>;
    
    /// Set autostart for a platform/account
    async fn set_autostart(&self, platform: &str, account_name: &str, enabled: bool) -> Result<(), Error>;
    
    /// Remove an autostart entry
    async fn remove_autostart(&self, platform: &str, account_name: &str) -> Result<(), Error>;
    
    /// Check if a platform/account is set to autostart
    async fn is_autostart_enabled(&self, platform: &str, account_name: &str) -> Result<bool, Error>;
}

#[derive(Clone)]
pub struct PostgresAutostartRepository {
    pool: Pool<Postgres>,
}

impl PostgresAutostartRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AutostartRepository for PostgresAutostartRepository {
    async fn get_enabled_entries(&self) -> Result<Vec<AutostartEntry>, Error> {
        let rows = sqlx::query_as::<_, AutostartEntry>(
            r#"
            SELECT id, platform, account_name, enabled, created_at, updated_at
            FROM autostart
            WHERE enabled = true
            ORDER BY platform, account_name
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows)
    }
    
    async fn get_all_entries(&self) -> Result<Vec<AutostartEntry>, Error> {
        let rows = sqlx::query_as::<_, AutostartEntry>(
            r#"
            SELECT id, platform, account_name, enabled, created_at, updated_at
            FROM autostart
            ORDER BY platform, account_name
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows)
    }
    
    async fn set_autostart(&self, platform: &str, account_name: &str, enabled: bool) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO autostart (platform, account_name, enabled)
            VALUES ($1, $2, $3)
            ON CONFLICT (platform, account_name)
            DO UPDATE SET 
                enabled = EXCLUDED.enabled,
                updated_at = NOW()
            "#
        )
        .bind(platform)
        .bind(account_name)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn remove_autostart(&self, platform: &str, account_name: &str) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM autostart
            WHERE platform = $1 AND account_name = $2
            "#
        )
        .bind(platform)
        .bind(account_name)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn is_autostart_enabled(&self, platform: &str, account_name: &str) -> Result<bool, Error> {
        let result: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT enabled
            FROM autostart
            WHERE platform = $1 AND account_name = $2
            "#
        )
        .bind(platform)
        .bind(account_name)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(result.map(|(enabled,)| enabled).unwrap_or(false))
    }
}