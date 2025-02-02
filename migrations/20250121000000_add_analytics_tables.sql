-- migrations/20250121000000_add_analytics_tables.sql (Postgres version)

-- 1) Chat Messages
CREATE TABLE IF NOT EXISTS chat_messages (
    message_id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,
    message_text TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
    metadata TEXT,
    CONSTRAINT fk_userid FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_platform_channel_timestamp
    ON chat_messages (platform, channel, timestamp);

-- 2) Bot Events
CREATE TABLE IF NOT EXISTS bot_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_timestamp BIGINT NOT NULL,
    data TEXT
);

CREATE INDEX IF NOT EXISTS idx_bot_events_type_time
    ON bot_events (event_type, event_timestamp);

-- 3) Plugins
CREATE TABLE IF NOT EXISTS plugins (
    plugin_id TEXT PRIMARY KEY,
    plugin_name TEXT NOT NULL,
    plugin_version TEXT,
    install_path TEXT,
    created_at BIGINT NOT NULL,
    last_connected_at BIGINT
);

-- 4) Plugin Events
CREATE TABLE IF NOT EXISTS plugin_events (
    event_id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_timestamp BIGINT NOT NULL,
    data TEXT,
    CONSTRAINT fk_plugin FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id)
);

CREATE INDEX IF NOT EXISTS idx_plugin_events_plugin_time
    ON plugin_events (plugin_id, event_timestamp);

-- 5) Command Logs
CREATE TABLE IF NOT EXISTS command_logs (
    command_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    channel TEXT,
    command_name TEXT NOT NULL,
    arguments TEXT,
    timestamp BIGINT NOT NULL,
    CONSTRAINT fk_userid FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_command_logs_user_time
    ON command_logs (user_id, timestamp);

-- 6) Moderation Actions
CREATE TABLE IF NOT EXISTS moderation_actions (
    action_id TEXT PRIMARY KEY,
    performed_by TEXT NOT NULL,
    affected_user TEXT NOT NULL,
    action_type TEXT NOT NULL,
    reason TEXT,
    timestamp BIGINT NOT NULL,
    CONSTRAINT fk_performed FOREIGN KEY (performed_by) REFERENCES users(user_id),
    CONSTRAINT fk_affected FOREIGN KEY (affected_user) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_moderation_actions_performed_by_time
    ON moderation_actions (performed_by, timestamp);

-- 7) Daily Stats
CREATE TABLE IF NOT EXISTS daily_stats (
    date TEXT PRIMARY KEY,
    total_messages BIGINT NOT NULL DEFAULT 0,
    unique_users BIGINT NOT NULL DEFAULT 0,
    total_commands BIGINT NOT NULL DEFAULT 0,
    total_mod_actions BIGINT NOT NULL DEFAULT 0,
    average_viewers BIGINT NOT NULL DEFAULT 0,
    max_viewers BIGINT NOT NULL DEFAULT 0,
    new_users BIGINT NOT NULL DEFAULT 0,
    returning_users BIGINT NOT NULL DEFAULT 0,
    total_chat_visits BIGINT NOT NULL DEFAULT 0
);

-- 8) Chat Sessions
CREATE TABLE IF NOT EXISTS chat_sessions (
    session_id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,
    joined_at BIGINT NOT NULL,
    left_at BIGINT NOT NULL,
    session_duration_seconds BIGINT,
    CONSTRAINT fk_userid FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_platform_channel
    ON chat_sessions (user_id, platform, channel);
