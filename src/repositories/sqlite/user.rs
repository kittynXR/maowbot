use super::*;
use crate::models::User;
use crate::repositories::Repository;
use crate::Error;
use chrono::NaiveDateTime;
use sqlx::{Pool, Sqlite, Row};

pub struct UserRepository {
    pool: Pool<Sqlite>
}

impl UserRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl Repository<User> for UserRepository {
    async fn create(&self, user: &User) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active, global_username)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
            .bind(&user.user_id)
            .bind(user.created_at)
            .bind(user.last_seen)
            .bind(user.is_active)
            .bind(&user.global_username)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<User>, Error> {
        let row = sqlx::query(
            r#"
            SELECT
                user_id,
                global_username,
                created_at,
                last_seen,
                is_active
            FROM users
            WHERE user_id = ?
            "#
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(User {
                user_id: r.try_get("user_id")?,
                global_username: r.try_get("global_username")?,
                created_at: r.try_get("created_at")?,
                last_seen: r.try_get("last_seen")?,
                is_active: r.try_get("is_active")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, user: &User) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE users
            SET last_seen = ?,
                is_active = ?,
                global_username = ?
            WHERE user_id = ?
            "#
        )
            .bind(user.last_seen)
            .bind(user.is_active)
            .bind(&user.global_username)
            .bind(&user.user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), Error> {
        sqlx::query("DELETE FROM users WHERE user_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
