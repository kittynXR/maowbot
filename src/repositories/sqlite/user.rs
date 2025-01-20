use super::*;
use crate::models::User;
use crate::repositories::Repository;
use chrono::NaiveDateTime;

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
        // Insert the new user, including global_username if provided
        sqlx::query!(
            r#"
            INSERT INTO users (user_id, created_at, last_seen, is_active, global_username)
            VALUES (?, ?, ?, ?, ?)
            "#,
            user.user_id,
            user.created_at,
            user.last_seen,
            user.is_active,         // if is_active is a real bool column in SQLite
            user.global_username
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<User>, Error> {
        // We'll explicitly tell SQLx that is_active is a bool
        let row = sqlx::query!(
            r#"
            SELECT
                user_id,
                created_at,
                last_seen,
                -- Tell SQLx: "is_active AS `is_active: bool`"
                is_active AS `is_active: bool`,
                global_username
            FROM users
            WHERE user_id = ?
            "#,
            id
        )
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(User {
                user_id: r.user_id,
                created_at: r.created_at,
                last_seen: r.last_seen,
                // Now `r.is_active` is already a bool, no need for != 0
                is_active: r.is_active,
                global_username: r.global_username,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, user: &User) -> Result<(), Error> {
        sqlx::query!(
            r#"
            UPDATE users
            SET last_seen = ?,
                is_active = ?,
                global_username = ?
            WHERE user_id = ?
            "#,
            user.last_seen,
            user.is_active,
            user.global_username,
            user.user_id
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
