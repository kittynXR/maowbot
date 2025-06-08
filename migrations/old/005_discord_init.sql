-- migrations/005_discord_init.sql
--
-- Creates tables to store Discord guilds, channels, and an “active server” pointer.

CREATE TABLE discord_accounts (
    account_name TEXT PRIMARY KEY,
    discord_id TEXT,
    credential_id UUID,
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2) Add a column for is_active on discord_guilds:
CREATE TABLE discord_guilds (
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
CREATE TABLE discord_channels (
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

CREATE TABLE discord_event_config (
    event_config_id             UUID NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    event_name                  TEXT NOT NULL,
    guild_id                    TEXT NOT NULL,
    channel_id                  TEXT NOT NULL,
    respond_with_credential     UUID NULL,
    ping_roles                  TEXT[] NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
CONSTRAINT discord_event_config_unique
UNIQUE (event_name, guild_id, channel_id, respond_with_credential)
);


-- Creates Discord live role table for assigning roles to users when they go live on Twitch

-- Table to store the role that should be assigned to Discord users who are streaming on Twitch
CREATE TABLE discord_live_roles (
    live_role_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    guild_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_live_role_per_guild UNIQUE(guild_id)
);

COMMENT ON TABLE discord_live_roles IS 'Stores Discord role IDs to assign to users when they are streaming on Twitch';
COMMENT ON COLUMN discord_live_roles.guild_id IS 'Discord guild/server ID';
COMMENT ON COLUMN discord_live_roles.role_id IS 'Discord role ID to assign to users when they are streaming on Twitch';
