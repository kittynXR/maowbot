// maowbot-core/src/repositories/postgres/discord.rs
//
// Provides a DiscordRepository trait and a PostgresDiscordRepository implementation
// that stores known guilds (servers) and channels in the “discord_guilds” and
// “discord_channels” tables. We also store a “active_server” column (or a separate table)
// keyed by (account_name), so we can retrieve the "active server" from the TUI.
//
// The assumption: whenever we see a GuildCreate or ChannelCreate/Update event in the
// Discord runtime, we call repository methods like upsert_guild(...) or upsert_channel(...),
// passing the “account_name” we’re using. That way, multiple different Discord accounts
// can track distinct sets of guilds.
//
// This data is updated from gateway events (since we cannot fetch the entire guild list on demand).
//

use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use std::time::SystemTime;
use chrono::{DateTime, Utc};
use maowbot_common::error::Error;
use maowbot_common::models::discord::{DiscordChannelRecord, DiscordGuildRecord};
use maowbot_common::traits::repository_traits::DiscordRepository;



/// Trait for storing and retrieving Discord guilds & channels from Postgres.


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
    async fn upsert_guild(&self, account_name: &str, guild_id: &str, guild_name: &str) -> Result<(), Error> {
        // Upsert into discord_guilds
        // If you store “active_server” in same table, add the column to your ON CONFLICT clause
        let q = r#"
            INSERT INTO discord_guilds (account_name, guild_id, guild_name)
            VALUES ($1, $2, $3)
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
            SELECT account_name, guild_id, guild_name, created_at, updated_at
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
                created_at:   r.try_get("created_at")?,
                updated_at:   r.try_get("updated_at")?,
            });
        }
        Ok(out)
    }

    async fn get_guild(&self, account_name: &str, guild_id: &str) -> Result<Option<DiscordGuildRecord>, Error> {
        let q = r#"
            SELECT account_name, guild_id, guild_name, created_at, updated_at
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
                created_at:   r.try_get("created_at")?,
                updated_at:   r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn upsert_channel(&self,
                            account_name: &str,
                            guild_id: &str,
                            channel_id: &str,
                            channel_name: &str
    ) -> Result<(), Error> {
        let q = r#"
            INSERT INTO discord_channels (account_name, guild_id, channel_id, channel_name)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (account_name, guild_id, channel_id)
            DO UPDATE SET channel_name = EXCLUDED.channel_name,
                          updated_at = now()
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
            SELECT account_name, guild_id, channel_id, channel_name, created_at, updated_at
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
                created_at:   r.try_get("created_at")?,
                updated_at:   r.try_get("updated_at")?,
            });
        }
        Ok(out)
    }

    async fn set_active_server(&self, account_name: &str, guild_id: &str) -> Result<(), Error> {
        // A simple approach: store in a separate table “discord_active_servers”
        // or add a column “is_active” to “discord_guilds”.
        // We’ll just do a second table for clarity. For demonstration:
        //
        //   REPLACE INTO discord_active_servers (account_name, guild_id, updated_at) ...
        //
        // But let's just do a small upsert. If you want a unique row, add a PK or uniqueness.

        let q_reset = r#"
            DELETE FROM discord_active_servers
            WHERE account_name = $1
        "#;
        sqlx::query(q_reset)
            .bind(account_name)
            .execute(&self.pool)
            .await?;

        let q_insert = r#"
            INSERT INTO discord_active_servers (account_name, guild_id, updated_at)
            VALUES ($1, $2, now())
        "#;
        sqlx::query(q_insert)
            .bind(account_name)
            .bind(guild_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_active_server(&self, account_name: &str) -> Result<Option<String>, Error> {
        let q = r#"
            SELECT guild_id
            FROM discord_active_servers
            WHERE account_name = $1
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
}

