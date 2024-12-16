// src/repositories/sqlite/user.rs

use super::*;
use crate::models::User;
use crate::repositories::Repository;

pub struct UserRepository {
    pool: Pool<Sqlite>
}

impl UserRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Repository<User> for UserRepository {
    async fn create(&self, user: &User) -> Result<(), Error> {
        sqlx::query!(
            r#"INSERT INTO users (user_id, created_at, last_seen, is_active)
            VALUES (?, ?, ?, ?)"#,
            user.user_id, user.created_at, user.last_seen, user.is_active
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<User>, Error> {
        let user = sqlx::query_as!(
            User,
            r#"SELECT user_id, created_at, last_seen, is_active
            FROM users WHERE user_id = ?"#,
            id
        )
            .fetch_optional(&self.pool)
            .await?;

        Ok(user)
    }

    async fn update(&self, user: &User) -> Result<(), Error> {
        sqlx::query!(
            r#"UPDATE users
            SET last_seen = ?, is_active = ?
            WHERE user_id = ?"#,
            user.last_seen, user.is_active, user.user_id
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), Error> {
        sqlx::query!("DELETE FROM users WHERE user_id = ?", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}