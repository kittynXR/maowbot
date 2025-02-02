// src/repositories/postgres/user_audit_log.rs

use crate::Error;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use crate::utils::time::{to_epoch, from_epoch};

#[derive(Debug, Clone)]
pub struct UserAuditLogEntry {
    pub audit_id: String,
    pub user_id: String,
    pub event_type: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_by: Option<String>,
    pub timestamp: i64,
    pub metadata: Option<String>,
}

impl UserAuditLogEntry {
    pub fn new(
        user_id: &str,
        event_type: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
        changed_by: Option<&str>,
        metadata: Option<&str>,
    ) -> Self {
        let now = to_epoch(Utc::now().naive_utc());
        Self {
            audit_id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            event_type: event_type.to_string(),
            old_value: old_value.map(String::from),
            new_value: new_value.map(String::from),
            changed_by: changed_by.map(String::from),
            timestamp: now,
            metadata: metadata.map(String::from),
        }
    }
}

#[async_trait]
pub trait UserAuditLogRepository {
    async fn insert_entry(&self, entry: &UserAuditLogEntry) -> Result<(), Error>;
    async fn get_entry(&self, audit_id: &str) -> Result<Option<UserAuditLogEntry>, Error>;
    async fn get_entries_for_user(&self, user_id: &str, limit: i64) -> Result<Vec<UserAuditLogEntry>, Error>;
}

#[derive(Clone)]
pub struct PostgresUserAuditLogRepository {
    pool: Pool<Postgres>,
}

impl PostgresUserAuditLogRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserAuditLogRepository for PostgresUserAuditLogRepository {
    async fn insert_entry(&self, entry: &UserAuditLogEntry) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO user_audit_log (
                audit_id, user_id, event_type,
                old_value, new_value, changed_by,
                timestamp, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
            .bind(&entry.audit_id)
            .bind(&entry.user_id)
            .bind(&entry.event_type)
            .bind(&entry.old_value)
            .bind(&entry.new_value)
            .bind(&entry.changed_by)
            .bind(entry.timestamp)
            .bind(&entry.metadata)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_entry(&self, audit_id: &str) -> Result<Option<UserAuditLogEntry>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                audit_id, user_id, event_type,
                old_value, new_value, changed_by,
                timestamp, metadata
            FROM user_audit_log
            WHERE audit_id = $1
            "#,
        )
            .bind(audit_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(UserAuditLogEntry {
                audit_id: r.try_get("audit_id")?,
                user_id: r.try_get("user_id")?,
                event_type: r.try_get("event_type")?,
                old_value: r.try_get("old_value")?,
                new_value: r.try_get("new_value")?,
                changed_by: r.try_get("changed_by")?,
                timestamp: r.try_get::<i64, _>("timestamp")?,
                metadata: r.try_get("metadata")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_entries_for_user(&self, user_id: &str, limit: i64)
                                  -> Result<Vec<UserAuditLogEntry>, Error>
    {
        let rows = sqlx::query(
            r#"
            SELECT
                audit_id, user_id, event_type,
                old_value, new_value, changed_by,
                timestamp, metadata
            FROM user_audit_log
            WHERE user_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for r in rows {
            results.push(UserAuditLogEntry {
                audit_id: r.try_get("audit_id")?,
                user_id: r.try_get("user_id")?,
                event_type: r.try_get("event_type")?,
                old_value: r.try_get("old_value")?,
                new_value: r.try_get("new_value")?,
                changed_by: r.try_get("changed_by")?,
                timestamp: r.try_get::<i64, _>("timestamp")?,
                metadata: r.try_get("metadata")?,
            });
        }
        Ok(results)
    }
}