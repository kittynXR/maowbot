// ========================================================
// File: maowbot-core/src/repositories/postgres/discord.rs
// ========================================================
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Pool, Postgres, Row, Transaction};
use tracing::{debug, warn};

// IMPORTANT: We need this import so &mut Transaction<'_, Postgres> implements Executor
use sqlx::Executor;

use maowbot_common::error::Error;
use maowbot_common::models::discord::{
    DiscordAccountRecord, DiscordChannelRecord, DiscordGuildRecord,
};
use maowbot_common::traits::repository_traits::DiscordRepository;

/// Implementation of DiscordRepository using Postgres.
#[derive(Clone)]
pub struct PostgresDiscordRepository {
    pool: Pool<Postgres>,
}

impl PostgresDiscordRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DiscordRepository for PostgresDiscordRepository {
    // ------------------------------------------------------------------------
    // Accounts
    // ------------------------------------------------------------------------
    async fn list_accounts(&self) -> Result<Vec<DiscordAccountRecord>, Error> {
        let q = r#"
            SELECT account_name, credential_id, is_active, created_at, updated_at
            FROM discord_accounts
            ORDER BY account_name
        "#;
        let rows = sqlx::query(q)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(DiscordAccountRecord {
                account_name: row.try_get("account_name")?,
                credential_id: row.try_get("credential_id").ok(), // If column is NULL
                is_active: row.try_get("is_active")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(results)
    }

    async fn upsert_account(&self, account_name: &str, maybe_credential: Option<uuid::Uuid>) -> Result<(), Error> {
        let q = r#"
            INSERT INTO discord_accounts (account_name, credential_id, is_active, created_at, updated_at)
            VALUES ($1, $2, false, now(), now())
            ON CONFLICT (account_name)
            DO UPDATE SET
                credential_id = EXCLUDED.credential_id,
                updated_at = now()
        "#;
        sqlx::query(q)
            .bind(account_name)
            .bind(maybe_credential)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn set_active_account(&self, account_name: &str) -> Result<(), Error> {
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        // 1) set is_active=false for all accounts
        sqlx::query("UPDATE discord_accounts SET is_active=false")
            .execute(&mut *tx) // <--- use &mut *tx
            .await?;

        // 2) set is_active=true for the specified account
        let rows_affected = sqlx::query(
            "UPDATE discord_accounts SET is_active=true, updated_at=now() WHERE account_name=$1"
        )
            .bind(account_name)
            .execute(&mut *tx) // <--- use &mut *tx
            .await?
            .rows_affected();

        if rows_affected == 0 {
            warn!("(set_active_account) No account found named '{}'", account_name);
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_active_account(&self) -> Result<Option<String>, Error> {
        let q = r#"
            SELECT account_name
            FROM discord_accounts
            WHERE is_active=true
            LIMIT 1
        "#;
        let row_opt = sqlx::query(q)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let name: String = row.try_get("account_name")?;
            Ok(Some(name))
        } else {
            Ok(None)
        }
    }

    // ------------------------------------------------------------------------
    // Guilds
    // ------------------------------------------------------------------------
    async fn upsert_guild(&self, account_name: &str, guild_id: &str, guild_name: &str) -> Result<(), Error> {
        let q = r#"
            INSERT INTO discord_guilds (account_name, guild_id, guild_name, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, false, now(), now())
            ON CONFLICT (account_name, guild_id)
            DO UPDATE SET guild_name = EXCLUDED.guild_name,
                          updated_at = now()
        "#;
        sqlx::query(q)
            .bind(account_name)
            .bind(guild_id)
            .bind(guild_name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_guilds_for_account(&self, account_name: &str) -> Result<Vec<DiscordGuildRecord>, Error> {
        let q = r#"
            SELECT account_name, guild_id, guild_name, is_active, created_at, updated_at
            FROM discord_guilds
            WHERE account_name = $1
            ORDER BY guild_name
        "#;
        let rows = sqlx::query(q)
            .bind(account_name)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::new();
        for r in rows {
            out.push(DiscordGuildRecord {
                account_name: r.try_get("account_name")?,
                guild_id:     r.try_get("guild_id")?,
                guild_name:   r.try_get("guild_name")?,
                is_active:    r.try_get("is_active")?,
                created_at:   r.try_get("created_at")?,
                updated_at:   r.try_get("updated_at")?,
            });
        }
        Ok(out)
    }

    async fn get_guild(&self, account_name: &str, guild_id: &str) -> Result<Option<DiscordGuildRecord>, Error> {
        let q = r#"
            SELECT account_name, guild_id, guild_name, is_active, created_at, updated_at
            FROM discord_guilds
            WHERE account_name = $1
              AND guild_id = $2
        "#;
        let row_opt = sqlx::query(q)
            .bind(account_name)
            .bind(guild_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            Ok(Some(DiscordGuildRecord {
                account_name: r.try_get("account_name")?,
                guild_id:     r.try_get("guild_id")?,
                guild_name:   r.try_get("guild_name")?,
                is_active:    r.try_get("is_active")?,
                created_at:   r.try_get("created_at")?,
                updated_at:   r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn set_active_server(&self, account_name: &str, guild_id: &str) -> Result<(), Error> {
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        // 1) set is_active=false for all guilds for this account
        sqlx::query(
            r#"
            UPDATE discord_guilds
            SET is_active=false
            WHERE account_name=$1
            "#,
        )
            .bind(account_name)
            .execute(&mut *tx)
            .await?;

        // 2) set is_active=true for the chosen guild
        let rows_affected = sqlx::query(
            r#"
            UPDATE discord_guilds
            SET is_active=true, updated_at=now()
            WHERE account_name=$1
              AND guild_id=$2
            "#,
        )
            .bind(account_name)
            .bind(guild_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        if rows_affected == 0 {
            warn!(
                "(set_active_server) No guild found for account='{}' with guild_id='{}'",
                account_name, guild_id
            );
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_active_server(&self, account_name: &str) -> Result<Option<String>, Error> {
        let q = r#"
            SELECT guild_id
            FROM discord_guilds
            WHERE account_name=$1
              AND is_active=true
            LIMIT 1
        "#;
        let row_opt = sqlx::query(q)
            .bind(account_name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let guild_id: String = r.try_get("guild_id")?;
            Ok(Some(guild_id))
        } else {
            Ok(None)
        }
    }

    // ------------------------------------------------------------------------
    // Channels
    // ------------------------------------------------------------------------
    async fn upsert_channel(&self,
                            account_name: &str,
                            guild_id: &str,
                            channel_id: &str,
                            channel_name: &str
    ) -> Result<(), Error> {
        let q = r#"
            INSERT INTO discord_channels (account_name, guild_id, channel_id, channel_name, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, false, now(), now())
            ON CONFLICT (account_name, guild_id, channel_id)
            DO UPDATE SET channel_name = EXCLUDED.channel_name,
                          updated_at   = now()
        "#;
        sqlx::query(q)
            .bind(account_name)
            .bind(guild_id)
            .bind(channel_id)
            .bind(channel_name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_channels_for_guild(&self,
                                     account_name: &str,
                                     guild_id: &str
    ) -> Result<Vec<DiscordChannelRecord>, Error> {
        let q = r#"
            SELECT account_name, guild_id, channel_id, channel_name, is_active, created_at, updated_at
            FROM discord_channels
            WHERE account_name = $1
              AND guild_id = $2
            ORDER BY channel_name
        "#;
        let rows = sqlx::query(q)
            .bind(account_name)
            .bind(guild_id)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::new();
        for r in rows {
            out.push(DiscordChannelRecord {
                account_name: r.try_get("account_name")?,
                guild_id:     r.try_get("guild_id")?,
                channel_id:   r.try_get("channel_id")?,
                channel_name: r.try_get("channel_name")?,
                is_active:    r.try_get("is_active")?,
                created_at:   r.try_get("created_at")?,
                updated_at:   r.try_get("updated_at")?,
            });
        }
        Ok(out)
    }

    async fn set_active_channel(&self, account_name: &str, guild_id: &str, channel_id: &str) -> Result<(), Error> {
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        // 1) set is_active=false for all channels in that guild
        sqlx::query(
            r#"
            UPDATE discord_channels
            SET is_active=false
            WHERE account_name=$1
              AND guild_id=$2
            "#,
        )
            .bind(account_name)
            .bind(guild_id)
            .execute(&mut *tx)
            .await?;

        // 2) set is_active=true for the chosen channel
        let rows_affected = sqlx::query(
            r#"
            UPDATE discord_channels
            SET is_active=true, updated_at=now()
            WHERE account_name=$1
              AND guild_id=$2
              AND channel_id=$3
            "#,
        )
            .bind(account_name)
            .bind(guild_id)
            .bind(channel_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        if rows_affected == 0 {
            warn!(
                "(set_active_channel) No channel found: account='{}', guild='{}', channel='{}'",
                account_name, guild_id, channel_id
            );
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_active_channel(&self, account_name: &str, guild_id: &str) -> Result<Option<String>, Error> {
        let q = r#"
            SELECT channel_id
            FROM discord_channels
            WHERE account_name=$1
              AND guild_id=$2
              AND is_active=true
            LIMIT 1
        "#;
        let row_opt = sqlx::query(q)
            .bind(account_name)
            .bind(guild_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row_opt {
            let ch_id: String = r.try_get("channel_id")?;
            Ok(Some(ch_id))
        } else {
            Ok(None)
        }
    }
}
