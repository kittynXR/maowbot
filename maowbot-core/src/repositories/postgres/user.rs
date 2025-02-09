// src/repositories/postgres/user.rs

use crate::models::User;
use crate::Error;
use sqlx::{Pool, Postgres, Row};
use chrono::{DateTime, Utc};

#[async_trait::async_trait]
pub trait UserRepo {
    async fn create(&self, user: &User) -> Result<(), Error>;
    async fn get(&self, id: &str) -> Result<Option<User>, Error>;
    async fn update(&self, user: &User) -> Result<(), Error>;
    async fn delete(&self, id: &str) -> Result<(), Error>;
    async fn list_all(&self) -> Result<Vec<User>, Error>;
}

pub struct UserRepository {
    pub pool: Pool<Postgres>,
}

impl UserRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl UserRepo for UserRepository {
    async fn create(&self, user: &User) -> Result<(), Error> {
        // Insert with TIMESTAMPTZ columns:
        sqlx::query(
            r#"
            INSERT INTO users (
                user_id, created_at, last_seen, is_active, global_username
            )
            VALUES ($1, $2, $3, $4, $5)
            "#,
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
            SELECT user_id,
                   global_username,
                   created_at,
                   last_seen,
                   is_active
            FROM users
            WHERE user_id = $1
            "#
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(User {
                user_id: r.try_get("user_id")?,
                global_username: r.try_get("global_username")?,
                created_at: r.try_get::<DateTime<Utc>, _>("created_at")?,
                last_seen: r.try_get::<DateTime<Utc>, _>("last_seen")?,
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
            SET global_username = $1,
                last_seen = $2,
                is_active = $3
            WHERE user_id = $4
            "#
        )
            .bind(&user.global_username)
            .bind(user.last_seen)
            .bind(user.is_active)
            .bind(&user.user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), Error> {
        sqlx::query("DELETE FROM users WHERE user_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<User>, Error> {
        // Because `User` is `#[derive(sqlx::FromRow)]` in your models,
        // we can use `query_as::<_, User>` directly:
        let rows = sqlx::query_as::<_, User>(
            r#"
            SELECT
                user_id,
                global_username,
                created_at,
                last_seen,
                is_active
            FROM users
            ORDER BY created_at ASC
            "#,
        )
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }
}