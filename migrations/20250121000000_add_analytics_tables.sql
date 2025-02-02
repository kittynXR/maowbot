-- migrations/20250121000000_add_analytics_tables.sql

-- 1) Chat Messages
CREATE TABLE IF NOT EXISTS chat_messages (
    message_id TEXT NOT NULL PRIMARY KEY,
    platform TEXT NOT NULL,      -- e.g. "twitch_helix", "discord"
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,       -- references users(user_id)
    message_text TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    metadata TEXT,               -- JSON for roles, badges, any ephemeral info
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_platform_channel_timestamp
    ON chat_messages (platform, channel, timestamp);

-- 2) Bot Events
CREATE TABLE IF NOT EXISTS bot_events (
    event_id TEXT NOT NULL PRIMARY KEY,
    event_type TEXT NOT NULL,    -- e.g. "plugin_connected", "system_start"
    event_timestamp INTEGER NOT NULL,
    data TEXT                    -- JSON details
);

CREATE INDEX IF NOT EXISTS idx_bot_events_type_time
    ON bot_events (event_type, event_timestamp);

-- 3) Plugins
CREATE TABLE IF NOT EXISTS plugins (
    plugin_id TEXT NOT NULL PRIMARY KEY,
    plugin_name TEXT NOT NULL,
    plugin_version TEXT,
    install_path TEXT,           -- if in-process .so/.dll
    created_at INTEGER NOT NULL,
    last_connected_at TIMESTAMP
);

-- 4) Plugin Events
CREATE TABLE IF NOT EXISTS plugin_events (
    event_id TEXT NOT NULL PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_timestamp INTEGER NOT NULL,
    data TEXT,                   -- JSON
    FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id)
);

CREATE INDEX IF NOT EXISTS idx_plugin_events_plugin_time
    ON plugin_events (plugin_id, event_timestamp);

-- 5) Command Logs
CREATE TABLE IF NOT EXISTS command_logs (
    command_id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL,       -- references users(user_id)
    platform TEXT NOT NULL,
    channel TEXT,
    command_name TEXT NOT NULL,
    arguments TEXT,              -- or could be JSON
    timestamp INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_command_logs_user_time
    ON command_logs (user_id, timestamp);

-- 6) Moderation Actions
CREATE TABLE IF NOT EXISTS moderation_actions (
    action_id TEXT NOT NULL PRIMARY KEY,
    performed_by TEXT NOT NULL,  -- references users(user_id)
    affected_user TEXT NOT NULL, -- references users(user_id)
    action_type TEXT NOT NULL,   -- e.g. "ban", "timeout"
    reason TEXT,
    timestamp INTEGER NOT NULL,
    FOREIGN KEY (performed_by) REFERENCES users(user_id),
    FOREIGN KEY (affected_user) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_moderation_actions_performed_by_time
    ON moderation_actions (performed_by, timestamp);

-- 7) Daily Stats
CREATE TABLE IF NOT EXISTS daily_stats (
    date TEXT NOT NULL PRIMARY KEY,           -- e.g. "YYYY-MM-DD"
    total_messages INTEGER NOT NULL DEFAULT 0,
    unique_users INTEGER NOT NULL DEFAULT 0,
    total_commands INTEGER NOT NULL DEFAULT 0,
    total_mod_actions INTEGER NOT NULL DEFAULT 0,
    average_viewers INTEGER NOT NULL DEFAULT 0,
    max_viewers INTEGER NOT NULL DEFAULT 0,
    new_users INTEGER NOT NULL DEFAULT 0,
    returning_users INTEGER NOT NULL DEFAULT 0,
    total_chat_visits INTEGER NOT NULL DEFAULT 0   -- # times users joined that day
);

-- 8) Chat Sessions: track user join/leave times in chat
CREATE TABLE IF NOT EXISTS chat_sessions (
    session_id TEXT NOT NULL PRIMARY KEY,
    platform TEXT NOT NULL,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,         -- references users(user_id)
    joined_at INTEGER NOT NULL,
    left_at INTEGER NOT NULL,
    session_duration_seconds INTEGER,  -- can be updated when user leaves
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_platform_channel
    ON chat_sessions (user_id, platform, channel);
