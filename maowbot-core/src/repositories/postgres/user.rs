use crate::Error;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use maowbot_common::models::user::User;
pub(crate) use maowbot_common::traits::repository_traits::UserRepo;

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
        sqlx::query(
            r#"
            INSERT INTO users (
                user_id, created_at, last_seen, is_active, global_username
            )
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
            .bind(user.user_id)
            .bind(user.created_at)
            .bind(user.last_seen)
            .bind(user.is_active)
            .bind(&user.global_username)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<User>, Error> {
        let row = sqlx::query(
            r#"
            SELECT user_id,
                   global_username,
                   created_at,
                   last_seen,
                   is_active
            FROM users
            WHERE user_id = $1
            "#,
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let user = User {
                user_id: r.try_get("user_id")?,
                global_username: r.try_get("global_username")?,
                created_at: r.try_get("created_at")?,
                last_seen: r.try_get("last_seen")?,
                is_active: r.try_get("is_active")?,
            };
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    // Updated to be case-insensitive:
    async fn get_by_global_username(&self, name: &str) -> Result<Option<User>, Error> {
        let row = sqlx::query(
            r#"
            SELECT user_id,
                   global_username,
                   created_at,
                   last_seen,
                   is_active
            FROM users
            WHERE LOWER(global_username) = LOWER($1)
            "#,
        )
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let user = User {
                user_id: r.try_get("user_id")?,
                global_username: r.try_get("global_username")?,
                created_at: r.try_get("created_at")?,
                last_seen: r.try_get("last_seen")?,
                is_active: r.try_get("is_active")?,
            };
            Ok(Some(user))
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
            .bind(user.last_seen)
            .bind(user.is_active)
            .bind(user.user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), Error> {
        sqlx::query("DELETE FROM users WHERE user_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<User>, Error> {
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

impl UserRepository {
    /// Find duplicate users based on similar usernames
    pub async fn find_duplicate_users(&self) -> Result<Vec<(String, Vec<User>)>, Error> {
        // Get all users with usernames
        let users = self.list_all().await?;
        
        // Group by lowercase username
        let mut groups: std::collections::HashMap<String, Vec<User>> = std::collections::HashMap::new();
        
        for user in users {
            if let Some(username) = &user.global_username {
                let key = username.to_lowercase();
                groups.entry(key).or_insert_with(Vec::new).push(user);
            }
        }
        
        // Filter to only groups with duplicates
        let duplicates: Vec<(String, Vec<User>)> = groups
            .into_iter()
            .filter(|(_, users)| users.len() > 1)
            .collect();
        
        Ok(duplicates)
    }
    
    /// Merge duplicate users by reassigning all platform identities to the primary user
    pub async fn merge_users(&self, primary_user_id: Uuid, duplicate_user_ids: Vec<Uuid>) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        
        for dup_id in duplicate_user_ids {
            // Update platform identities to point to primary user
            sqlx::query(
                "UPDATE platform_identities SET user_id = $1 WHERE user_id = $2"
            )
                .bind(primary_user_id)
                .bind(dup_id)
                .execute(&mut *tx)
                .await?;
            
            // Update command usage
            sqlx::query(
                "UPDATE command_usage SET user_id = $1 WHERE user_id = $2"
            )
                .bind(primary_user_id)
                .bind(dup_id)
                .execute(&mut *tx)
                .await?;
            
            // Update redeem usage
            sqlx::query(
                "UPDATE redeem_usage SET user_id = $1 WHERE user_id = $2"
            )
                .bind(primary_user_id)
                .bind(dup_id)
                .execute(&mut *tx)
                .await?;
            
            // Update user analysis (delete duplicates, keep primary)
            sqlx::query(
                "DELETE FROM user_analysis WHERE user_id = $1"
            )
                .bind(dup_id)
                .execute(&mut *tx)
                .await?;
            
            // Update user audit logs
            sqlx::query(
                "UPDATE user_audit_logs SET user_id = $1 WHERE user_id = $2"
            )
                .bind(primary_user_id)
                .bind(dup_id)
                .execute(&mut *tx)
                .await?;
            
            // Finally, delete the duplicate user
            sqlx::query(
                "DELETE FROM users WHERE user_id = $1"
            )
                .bind(dup_id)
                .execute(&mut *tx)
                .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }
}