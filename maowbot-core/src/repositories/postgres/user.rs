// src/repositories/postgres/user.rs

use crate::models::User;
use crate::utils::time::{to_epoch, from_epoch, current_epoch};
use crate::Error;
use sqlx::{Pool, Postgres, Row};

#[async_trait::async_trait]
pub trait UserRepo {
    async fn create(&self, user: &User) -> Result<(), Error>;
    async fn get(&self, id: &str) -> Result<Option<User>, Error>;
    async fn update(&self, user: &User) -> Result<(), Error>;
    async fn delete(&self, id: &str) -> Result<(), Error>;
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
        // Insert with placeholders $1..$5
        sqlx::query(
            r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active, global_username)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
            .bind(&user.user_id)
            .bind(to_epoch(user.created_at))
            .bind(to_epoch(user.last_seen))
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
            WHERE user_id = $1
            "#,
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
            SET global_username = $1,
                last_seen = $2,
                is_active = $3
            WHERE user_id = $4
            "#,
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
        sqlx::query("DELETE FROM users WHERE user_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}