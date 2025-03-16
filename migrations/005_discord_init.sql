-- migrations/005_discord_init.sql
--
-- Creates tables to store Discord guilds, channels, and an “active server” pointer.

CREATE TABLE IF NOT EXISTS discord_accounts (
    account_name TEXT PRIMARY KEY,
    credential_id UUID,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2) Add a column for is_active on discord_guilds:
CREATE TABLE IF NOT EXISTS discord_guilds (
    account_name TEXT NOT NULL,
    guild_id     TEXT NOT NULL,
    guild_name   TEXT NOT NULL,
    is_active    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

CONSTRAINT pk_discord_guilds PRIMARY KEY (account_name, guild_id),
CONSTRAINT fk_discord_accounts
FOREIGN KEY (account_name) REFERENCES discord_accounts(account_name)
ON DELETE CASCADE
);

-- 3) Add a column for is_active on discord_channels:
CREATE TABLE IF NOT EXISTS discord_channels (
    account_name TEXT NOT NULL,
    guild_id     TEXT NOT NULL,
    channel_id   TEXT NOT NULL,
    channel_name TEXT NOT NULL,
    is_active    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

CONSTRAINT pk_discord_channels PRIMARY KEY (account_name, guild_id, channel_id),
CONSTRAINT fk_discord_guilds
FOREIGN KEY (account_name, guild_id)
REFERENCES discord_guilds (account_name, guild_id)
ON DELETE CASCADE
);