// ========================================================
// File: maowbot-core/src/repositories/postgres/discord.rs
// ========================================================
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Pool, Postgres, Row, Transaction};
use tracing::{debug, warn, info};

use sqlx::Executor;

use maowbot_common::error::Error;
use maowbot_common::models::discord::{
    DiscordAccountRecord,
    DiscordChannelRecord,
    DiscordGuildRecord,
    DiscordEventConfigRecord,
};
use maowbot_common::traits::repository_traits::DiscordRepository;

#[derive(Clone)]
pub struct PostgresDiscordRepository {
    pool: Pool<Postgres>,
}

impl PostgresDiscordRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn upsert_event_config(
        &self,
        event_name: &str,
        guild_id: &str,
        channel_id: &str,
        respond_with_credential: Option<uuid::Uuid>,
    ) -> Result<(), Error> {
        let q = r#"
            INSERT INTO discord_event_config (
                event_config_id,
                event_name,
                guild_id,
                channel_id,
                respond_with_credential,
                created_at,
                updated_at
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, NOW(), NOW())
            ON CONFLICT (event_name, guild_id, channel_id, respond_with_credential)
            DO UPDATE SET
                guild_id = EXCLUDED.guild_id,
                channel_id = EXCLUDED.channel_id,
                respond_with_credential = EXCLUDED.respond_with_credential,
                updated_at = NOW()
        "#;
        sqlx::query(q)
            .bind(event_name)
            .bind(guild_id)
            .bind(channel_id)
            .bind(respond_with_credential)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_event_config_by_name(
        &self,
        event_name: &str
    ) -> Result<Option<DiscordEventConfigRecord>, Error> {
        let q = r#"
            SELECT event_config_id,
                   event_name,
                   guild_id,
                   channel_id,
                   respond_with_credential,
                   ping_roles,
                   created_at,
                   updated_at
            FROM discord_event_config
            WHERE event_name = $1
            LIMIT 1
        "#;
        let row_opt = sqlx::query(q)
            .bind(event_name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row_opt {
            let record = DiscordEventConfigRecord {
                event_config_id: row.try_get("event_config_id")?,
                event_name: row.try_get("event_name")?,
                guild_id: row.try_get("guild_id")?,
                channel_id: row.try_get("channel_id")?,
                respond_with_credential: row.try_get("respond_with_credential").ok(),
                ping_roles: row.try_get("ping_roles").ok(),
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    pub async fn list_event_configs(&self) -> Result<Vec<DiscordEventConfigRecord>, Error> {
        let q = r#"
            SELECT event_config_id,
                   event_name,
                   guild_id,
                   channel_id,
                   respond_with_credential,
                   ping_roles,
                   created_at,
                   updated_at
            FROM discord_event_config
            ORDER BY event_name
        "#;
        let rows = sqlx::query(q)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::new();
        for row in rows {
            out.push(DiscordEventConfigRecord {
                event_config_id: row.try_get("event_config_id")?,
                event_name: row.try_get("event_name")?,
                guild_id: row.try_get("guild_id")?,
                channel_id: row.try_get("channel_id")?,
                respond_with_credential: row.try_get("respond_with_credential").ok(),
                ping_roles: row.try_get("ping_roles").ok(),
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(out)
    }

    // -------------------------------------------------------------------------
    // NEW: multi-row approach for storing Discord event configs (existing functions)
    // -------------------------------------------------------------------------
    pub async fn insert_event_config_multi(
        &self,
        event_name: &str,
        guild_id: &str,
        channel_id: &str,
        respond_with_credential: Option<uuid::Uuid>,
    ) -> Result<(), Error> {
        let q = r#"
            INSERT INTO discord_event_config (
                event_config_id,
                event_name,
                guild_id,
                channel_id,
                respond_with_credential,
                created_at,
                updated_at
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, now(), now())
            ON CONFLICT (event_name, guild_id, channel_id, respond_with_credential)
            DO UPDATE SET
                updated_at = now()
        "#;
        sqlx::query(q)
            .bind(event_name)
            .bind(guild_id)
            .bind(channel_id)
            .bind(respond_with_credential)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_event_config_multi(
        &self,
        event_name: &str,
        guild_id: &str,
        channel_id: &str,
        respond_with_credential: Option<uuid::Uuid>,
    ) -> Result<(), Error> {
        let q = r#"
            DELETE FROM discord_event_config
            WHERE event_name = $1
              AND guild_id   = $2
              AND channel_id = $3
              AND (($4 IS NULL AND respond_with_credential IS NULL)
                   OR respond_with_credential = $4)
        "#;
        sqlx::query(q)
            .bind(event_name)
            .bind(guild_id)
            .bind(channel_id)
            .bind(respond_with_credential)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // NEW: Functions to manage ping_roles in the discord_event_config table
    // -------------------------------------------------------------------------
    pub async fn add_event_config_role(
        &self,
        event_name: &str,
        role_id: &str,
    ) -> Result<(), Error> {
        let q = r#"
            UPDATE discord_event_config
            SET ping_roles =
                CASE
                    WHEN ping_roles IS NULL THEN ARRAY[$2]
                    WHEN NOT ($2 = ANY(ping_roles)) THEN array_append(ping_roles, $2)
                    ELSE ping_roles
                END,
                updated_at = NOW()
            WHERE event_name = $1
        "#;
        sqlx::query(q)
            .bind(event_name)
            .bind(role_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_event_config_role(
        &self,
        event_name: &str,
        role_id: &str,
    ) -> Result<(), Error> {
        let q = r#"
            UPDATE discord_event_config
            SET ping_roles =
                CASE
                    WHEN ping_roles IS NOT NULL THEN array_remove(ping_roles, $2)
                    ELSE NULL
                END,
                updated_at = NOW()
            WHERE event_name = $1
        "#;
        sqlx::query(q)
            .bind(event_name)
            .bind(role_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// NEW: Dummy implementation for listing roles for a guild.
    /// In production, this should call the Discord API.
    pub async fn list_roles_for_guild(&self, guild_id: &str) -> Result<Vec<(String, String)>, Error> {
        // For now, return an empty list.
        Ok(vec![])
    }
}

// =================================================================================================
// Implementation of the DiscordRepository trait
// =================================================================================================
#[async_trait]
impl maowbot_common::traits::repository_traits::DiscordRepository for PostgresDiscordRepository {
    // ------------------------------------------------------------------------
    // Accounts
    // ------------------------------------------------------------------------
    async fn list_accounts(&self) -> Result<Vec<DiscordAccountRecord>, Error> {
        let q = r#"
            SELECT account_name,
                   credential_id,
                   is_active,
                   created_at,
                   updated_at,
                   discord_id
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
                credential_id: row.try_get("credential_id").ok(),
                is_active: row.try_get("is_active")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
                discord_id: row.try_get("discord_id").ok(),
            });
        }
        Ok(results)
    }

    // UPDATED METHOD: now has an extra parameter for `discord_id`.
    async fn upsert_account(&self, account_name: &str, maybe_credential: Option<uuid::Uuid>, discord_id: Option<&str>) -> Result<(), Error> {
        let q = r#"
            INSERT INTO discord_accounts (account_name, credential_id, is_active, created_at, updated_at, discord_id)
            VALUES ($1, $2, false, now(), now(), $3)
            ON CONFLICT (account_name)
            DO UPDATE SET
                credential_id = EXCLUDED.credential_id,
                updated_at = now(),
                discord_id = EXCLUDED.discord_id
        "#;
        sqlx::query(q)
            .bind(account_name)
            .bind(maybe_credential)
            .bind(discord_id)
            .execute(&self.pool)
            .await?;

        let count_q = r#"SELECT COUNT(*) AS cnt FROM discord_accounts"#;
        let total_count: i64 = sqlx::query_scalar(count_q)
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        if total_count == 1 {
            let _ = sqlx::query(
                "UPDATE discord_accounts SET is_active=true"
            )
                .execute(&self.pool)
                .await?;
            info!("(upsert_account) Only one Discord account detected => set is_active=true");
        }

        Ok(())
    }

    async fn set_active_account(&self, account_name: &str) -> Result<(), Error> {
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        sqlx::query("UPDATE discord_accounts SET is_active=false")
            .execute(&mut *tx)
            .await?;

        let rows_affected = sqlx::query(
            "UPDATE discord_accounts SET is_active=true, updated_at=now() WHERE account_name=$1"
        )
            .bind(account_name)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        if rows_affected == 0 {
            warn!("(set_active_account) No existing row for account_name='{account_name}'. Will create new row with is_active=true.");

            let ins_q = r#"
            INSERT INTO discord_accounts (account_name, credential_id, is_active, created_at, updated_at, discord_id)
            VALUES ($1, NULL, true, now(), now(), NULL)
            ON CONFLICT (account_name)
            DO UPDATE SET is_active=true, updated_at=now()
            "#;
            sqlx::query(ins_q)
                .bind(account_name)
                .execute(&mut *tx)
                .await?;
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

        let count_q = r#"
            SELECT COUNT(*) AS cnt
            FROM discord_guilds
            WHERE account_name = $1
        "#;
        let total_guilds_for_acct: i64 = sqlx::query_scalar(count_q)
            .bind(account_name)
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        if total_guilds_for_acct == 1 {
            let _ = sqlx::query(
                "UPDATE discord_guilds SET is_active=true WHERE account_name=$1"
            )
                .bind(account_name)
                .execute(&self.pool)
                .await?;
            info!(
                "(upsert_guild) For account='{account_name}', only 1 guild => set is_active=true."
            );
        }

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

        sqlx::query(
            r#"
            UPDATE discord_guilds
            SET is_active=false
            WHERE account_name=$1
            "#
        )
            .bind(account_name)
            .execute(&mut *tx)
            .await?;

        let rows_affected = sqlx::query(
            r#"
            UPDATE discord_guilds
            SET is_active=true, updated_at=now()
            WHERE account_name=$1
              AND guild_id=$2
            "#
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

        sqlx::query(
            r#"
            UPDATE discord_channels
            SET is_active=false
            WHERE account_name=$1
              AND guild_id=$2
            "#
        )
            .bind(account_name)
            .bind(guild_id)
            .execute(&mut *tx)
            .await?;

        let rows_affected = sqlx::query(
            r#"
            UPDATE discord_channels
            SET is_active=true, updated_at=now()
            WHERE account_name=$1
              AND guild_id=$2
              AND channel_id=$3
            "#
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
