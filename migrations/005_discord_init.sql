-- migrations/005_discord_init.sql
--
-- Creates tables to store Discord guilds, channels, and an “active server” pointer.

CREATE TABLE IF NOT EXISTS discord_guilds (
    account_name TEXT NOT NULL,
    guild_id     TEXT NOT NULL,
    guild_name   TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_name, guild_id)
);

CREATE TABLE IF NOT EXISTS discord_channels (
    account_name TEXT NOT NULL,
    guild_id     TEXT NOT NULL,
    channel_id   TEXT NOT NULL,
    channel_name TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_name, guild_id, channel_id)
);

-- Stores which guild is “active” for the given account.
CREATE TABLE IF NOT EXISTS discord_active_servers (
    account_name TEXT PRIMARY KEY,
    guild_id     TEXT NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
