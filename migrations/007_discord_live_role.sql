-- migrations/007_discord_live_role.sql
--
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