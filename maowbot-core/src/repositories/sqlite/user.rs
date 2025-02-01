// src/repositories/sqlite/user.rs

use crate::utils::time::{to_epoch, from_epoch, current_epoch};
use crate::models::User;
use crate::Error;
use sqlx::Row;

pub struct UserRepository {
    pub pool: sqlx::Pool<sqlx::Sqlite>,
}

impl UserRepository {
    /// Constructor for UserRepository.
    pub fn new(pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl crate::repositories::Repository<User> for UserRepository {
    async fn create(&self, user: &User) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active, global_username)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
            .bind(&user.user_id)
            .bind(current_epoch())
            .bind(current_epoch())
            .bind(user.is_active)
            .bind(&user.global_username)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<User>, Error> {
        let row = sqlx::query(
            r#"
            SELECT user_id, global_username, created_at, last_seen, is_active
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
                created_at: from_epoch(r.try_get::<i64, _>("created_at")?),
                last_seen: from_epoch(r.try_get::<i64, _>("last_seen")?),
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
            SET global_username = ?,
                last_seen = ?,
                is_active = ?
            WHERE user_id = ?
            "#
        )
            .bind(&user.global_username)
            .bind(to_epoch(user.last_seen))
            .bind(user.is_active)
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