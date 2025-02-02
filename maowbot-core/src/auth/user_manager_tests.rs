// tests/user_manager_tests.rs

use sqlx::{Pool, Postgres};
use sqlx::postgres::PgPoolOptions;
use crate::Error;

mod tests {
    use uuid::Uuid;
    use std::env;
    use crate::Database;
    use crate::models::{Platform, User};
    use crate::repositories::postgres::{
        platform_identity::PlatformIdentityRepository,
        user::UserRepository,
        user_analysis::PostgresUserAnalysisRepository,
    };
    use crate::auth::{DefaultUserManager, UserManager};
    use crate::Error;
    use crate::auth::user_manager_tests::{clean_database, create_test_db_pool};

    /// A helper to create a test Postgres DB connection, run migrations, and build a DefaultUserManager.
    async fn setup_user_manager() -> Result<DefaultUserManager, Error> {
        let pool = create_test_db_pool().await?;

        // 2) Wipe the schema or data if you want a clean start
        clean_database(&pool).await?;

        // 3) Now wrap in your "Database" struct
        let db = Database::from_pool(pool.clone());
        db.migrate().await?;

        // 3) Create the repositories.
        let user_repo = UserRepository::new(db.pool().clone());
        let ident_repo = PlatformIdentityRepository::new(db.pool().clone());
        let analysis_repo = PostgresUserAnalysisRepository::new(db.pool().clone());

        // 4) Build the user manager.
        Ok(DefaultUserManager::new(user_repo, ident_repo, analysis_repo))
    }

    #[tokio::test]
    async fn test_get_or_create_user() -> Result<(), Error> {
        let manager = setup_user_manager().await?;
        let random_id = Uuid::new_v4().to_string();

        let user = manager
            .get_or_create_user(Platform::Discord, &random_id, Some("testuser"))
            .await?;
        assert!(!user.user_id.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_get_or_create_user_cache_hit() -> Result<(), Error> {
        let manager = setup_user_manager().await?;

        let user_id = Uuid::new_v4().to_string(); // random platform ID
        let user = manager
            .get_or_create_user(Platform::Twitch, &user_id, Some("TwitchDude"))
            .await?;

        // The first call hits the DB; the second call is the "cache hit".
        let user2 = manager
            .get_or_create_user(Platform::Twitch, &user_id, Some("NewName"))
            .await?;

        // Confirm it returns the same user (no new DB record).
        assert_eq!(user.user_id, user2.user_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_user_activity() -> Result<(), Error> {
        let manager = setup_user_manager().await?;
        let user = manager
            .get_or_create_user(Platform::VRChat, "vrchat_123", Some("VRChatter"))
            .await?;
        let old_seen = user.last_seen;

        // Wait 1 second
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Update with new username
        manager
            .update_user_activity(&user.user_id, Some("VRChatterNew"))
            .await?;

        let updated_user = manager
            .get_or_create_user(Platform::VRChat, "vrchat_123", None)
            .await?;

        assert!(
            updated_user.last_seen > old_seen,
            "Should have a more recent last_seen"
        );
        assert_eq!(
            updated_user.global_username,
            Some("VRChatterNew".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_ttl_expiration() -> Result<(), Error> {
        let manager = setup_user_manager().await?;

        // Insert user in the cache by calling get_or_create_user
        let user = manager
            .get_or_create_user(Platform::Discord, "some_discord_id", Some("DiscordTest"))
            .await?;

        // Force the last_access to 25 hours ago (simulate an expired cache entry).
        let changed = manager
            .test_force_last_access(Platform::Discord, "some_discord_id", 25)
            .await;
        assert!(changed, "Should have updated the existing cache entry");

        // Now calling get_or_create_user should prune that stale entry first,
        // then re-insert. We'll confirm it's still the same DB user though:
        let user2 = manager
            .get_or_create_user(Platform::Discord, "some_discord_id", Some("DiscordTest2"))
            .await?;

        // Both user + user2 should be the same DB user, but the old cache entry was removed internally.
        assert_eq!(user.user_id, user2.user_id);
        Ok(())
    }
}

pub async fn create_test_db_pool() -> Result<Pool<Postgres>, Error> {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://maow@localhost/maowbot".to_string());

    // We can allow only 1 connection if you want to ensure a serial approach:
    // .max_connections(1)
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    Ok(pool)
}

pub async fn clean_database(pool: &Pool<Postgres>) -> Result<(), Error> {
    // We list all known tables, and “TRUNCATE ... RESTART IDENTITY CASCADE”
    // so we clear them out for a fresh test run.
    // For example:
    sqlx::query(r#"
        TRUNCATE TABLE
            users,
            platform_identities,
            platform_credentials,
            user_analysis,
            link_requests,
            user_audit_log,
            daily_stats,
            chat_sessions,
            bot_events,
            command_logs,
            chat_messages,
            plugin_events,
            user_analysis_history,
            maintenance_state
        RESTART IDENTITY CASCADE;
    "#)
        .execute(pool)
        .await?;

    Ok(())
}
