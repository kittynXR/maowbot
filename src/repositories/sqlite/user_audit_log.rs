// src/repositories/sqlite/user_audit_log.rs

use sqlx::{Pool, Sqlite};
use crate::Error;
use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

/// Reflects one row in the `user_audit_log` table
#[derive(Debug, Clone)]
pub struct UserAuditLogEntry {
    pub audit_id: String,
    pub user_id: String,      // The user who was changed
    pub event_type: String,   // e.g. "name_change", "link_approved"
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_by: Option<String>,
    pub timestamp: NaiveDateTime,
    pub metadata: Option<String>,
}

impl UserAuditLogEntry {
    /// Helper to create a new log entry
    pub fn new(
        user_id: &str,
        event_type: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
        changed_by: Option<&str>,
        metadata: Option<&str>,
    ) -> Self {
        Self {
            audit_id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            event_type: event_type.to_string(),
            old_value: old_value.map(|s| s.to_string()),
            new_value: new_value.map(|s| s.to_string()),
            changed_by: changed_by.map(|s| s.to_string()),
            timestamp: Utc::now().naive_utc(),
            metadata: metadata.map(|s| s.to_string()),
        }
    }
}

/// Minimal trait for user_audit_log
#[async_trait]
pub trait UserAuditLogRepository {
    async fn insert_entry(&self, entry: &UserAuditLogEntry) -> Result<(), Error>;
    async fn get_entry(&self, audit_id: &str) -> Result<Option<UserAuditLogEntry>, Error>;
    async fn get_entries_for_user(&self, user_id: &str, limit: i64)
                                  -> Result<Vec<UserAuditLogEntry>, Error>;
}

/// Concrete implementation for SQLite
#[derive(Clone)]
pub struct SqliteUserAuditLogRepository {
    pool: Pool<Sqlite>,
}

impl SqliteUserAuditLogRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserAuditLogRepository for SqliteUserAuditLogRepository {
    async fn insert_entry(&self, entry: &UserAuditLogEntry) -> Result<(), Error> {
        sqlx::query!(
            r#"
            INSERT INTO user_audit_log (
                audit_id, user_id, event_type,
                old_value, new_value, changed_by,
                timestamp, metadata
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            entry.audit_id,
            entry.user_id,
            entry.event_type,
            entry.old_value,
            entry.new_value,
            entry.changed_by,
            entry.timestamp,
            entry.metadata
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_entry(&self, audit_id: &str) -> Result<Option<UserAuditLogEntry>, Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                audit_id, user_id, event_type,
                old_value, new_value, changed_by,
                timestamp, metadata
            FROM user_audit_log
            WHERE audit_id = ?
            "#,
            audit_id
        )
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(UserAuditLogEntry {
                audit_id: r.audit_id,
                user_id: r.user_id,
                event_type: r.event_type,
                old_value: r.old_value,
                new_value: r.new_value,
                changed_by: r.changed_by,
                timestamp: r.timestamp,
                metadata: r.metadata,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_entries_for_user(&self, user_id: &str, limit: i64)
                                  -> Result<Vec<UserAuditLogEntry>, Error>
    {
        let rows = sqlx::query!(
            r#"
            SELECT
                audit_id, user_id, event_type,
                old_value, new_value, changed_by,
                timestamp, metadata
            FROM user_audit_log
            WHERE user_id = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
            user_id,
            limit
        )
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for r in rows {
            results.push(UserAuditLogEntry {
                audit_id: r.audit_id,
                user_id: r.user_id,
                event_type: r.event_type,
                old_value: r.old_value,
                new_value: r.new_value,
                changed_by: r.changed_by,
                timestamp: r.timestamp,
                metadata: r.metadata,
            });
        }
        Ok(results)
    }
}
