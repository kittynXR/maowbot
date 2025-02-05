-- ----------------------------------------------------------------
-- Single flattened migration for Postgres
-- using TIMESTAMPTZ instead of BIGINT for timestamps
-- ----------------------------------------------------------------

-- 1) Users
CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    global_username TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

-- 2) Platform Identities
CREATE TABLE IF NOT EXISTS platform_identities (
    platform_identity_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    platform_user_id TEXT NOT NULL,
    platform_username TEXT NOT NULL,
    platform_display_name TEXT,
    platform_roles TEXT NOT NULL,    -- JSON array
    platform_data TEXT NOT NULL,     -- JSON object
    created_at TIMESTAMPTZ NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_user
    FOREIGN KEY (user_id) REFERENCES users(user_id),
    UNIQUE (platform, platform_user_id)
);

CREATE INDEX IF NOT EXISTS idx_platform_identities_user
    ON platform_identities (user_id);

CREATE INDEX IF NOT EXISTS idx_platform_identities_platform
    ON platform_identities (platform, platform_user_id);

-- 3) Platform Credentials
CREATE TABLE IF NOT EXISTS platform_credentials (
    credential_id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    credential_type TEXT NOT NULL,
    user_id TEXT NOT NULL,
    primary_token TEXT NOT NULL,
    refresh_token TEXT,
    additional_data TEXT,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    is_bot BOOLEAN NOT NULL DEFAULT false,
    CONSTRAINT fk_userid
    FOREIGN KEY (user_id) REFERENCES users(user_id),
    UNIQUE (platform, user_id)
);

CREATE INDEX IF NOT EXISTS idx_platform_credentials_user
    ON platform_credentials (user_id, platform);

CREATE TABLE IF NOT EXISTS app_config (
    config_key TEXT PRIMARY KEY,
    config_value TEXT
);

-- 4) Analytics-Related Tables
CREATE TABLE IF NOT EXISTS bot_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_timestamp TIMESTAMPTZ NOT NULL,
    data TEXT
);

CREATE INDEX IF NOT EXISTS idx_bot_events_type_time
    ON bot_events (event_type, event_timestamp);

CREATE TABLE IF NOT EXISTS plugins (
    plugin_id TEXT PRIMARY KEY,
    plugin_name TEXT NOT NULL,
    plugin_version TEXT,
    install_path TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    last_connected_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS plugin_events (
    event_id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_timestamp TIMESTAMPTZ NOT NULL,
    data TEXT,
    CONSTRAINT fk_plugin
    FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id)
);

CREATE INDEX IF NOT EXISTS idx_plugin_events_plugin_time
    ON plugin_events (plugin_id, event_timestamp);

CREATE TABLE IF NOT EXISTS command_logs (
    command_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    channel TEXT,
    command_name TEXT NOT NULL,
    arguments TEXT,
    timestamp TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_userid
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_command_logs_user_time
    ON command_logs (user_id, timestamp);

CREATE TABLE IF NOT EXISTS moderation_actions (
    action_id TEXT PRIMARY KEY,
    performed_by TEXT NOT NULL,
    affected_user TEXT NOT NULL,
    action_type TEXT NOT NULL,
    reason TEXT,
    timestamp TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_performed
    FOREIGN KEY (performed_by) REFERENCES users(user_id),
    CONSTRAINT fk_affected
    FOREIGN KEY (affected_user) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_moderation_actions_performed_by_time
    ON moderation_actions (performed_by, timestamp);

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

CREATE TABLE IF NOT EXISTS chat_sessions (
    session_id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,
    joined_at TIMESTAMPTZ NOT NULL,
    left_at TIMESTAMPTZ,
    session_duration_seconds BIGINT,
    CONSTRAINT fk_userid
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_platform_channel
    ON chat_sessions (user_id, platform, channel);

-- 5) Link Requests / User Audit Log
CREATE TABLE IF NOT EXISTS link_requests (
    link_request_id TEXT PRIMARY KEY,
    requesting_user_id TEXT NOT NULL,
    target_platform TEXT,
    target_platform_user_id TEXT,
    link_code TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_requesting_user
    FOREIGN KEY (requesting_user_id) REFERENCES users(user_id)
);

CREATE TABLE IF NOT EXISTS user_audit_log (
    audit_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    changed_by TEXT,
    timestamp TIMESTAMPTZ NOT NULL,
    metadata TEXT,
    CONSTRAINT fk_user
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

-- 6) user_analysis
CREATE TABLE IF NOT EXISTS user_analysis (
    user_analysis_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    spam_score REAL NOT NULL DEFAULT 0,
    intelligibility_score REAL NOT NULL DEFAULT 0,
    quality_score REAL NOT NULL DEFAULT 0,
    horni_score REAL NOT NULL DEFAULT 0,
    ai_notes TEXT,
    moderator_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_user
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_analysis_user
    ON user_analysis (user_id);

-- 7) user_analysis_history & maintenance_state
CREATE TABLE IF NOT EXISTS user_analysis_history (
    user_analysis_history_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    year_month TEXT NOT NULL,
    spam_score REAL NOT NULL DEFAULT 0,
    intelligibility_score REAL NOT NULL DEFAULT 0,
    quality_score REAL NOT NULL DEFAULT 0,
    horni_score REAL NOT NULL DEFAULT 0,
    ai_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_user
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_analysis_history_user_month
    ON user_analysis_history (user_id, year_month);

CREATE TABLE IF NOT EXISTS maitenance_state (
    state_key TEXT PRIMARY KEY,
    state_value TEXT
);

-- 8) Partitioned chat_messages

------------------------------------------------------------------
-- Chat messages: Now define it as PARTITIONED by timestamp
------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS chat_messages (
    timestamp TIMESTAMPTZ NOT NULL,
    message_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,
    message_text TEXT,
    metadata TEXT,
    CONSTRAINT fk_userid FOREIGN KEY (user_id) REFERENCES users(user_id),
    CONSTRAINT chat_messages_pk PRIMARY KEY (timestamp, message_id)
)
PARTITION BY RANGE (timestamp);

-- Create a default partition to catch rows whose timestamps aren’t covered by any explicit partition:
CREATE TABLE IF NOT EXISTS chat_messages_default
    PARTITION OF chat_messages DEFAULT;

CREATE INDEX IF NOT EXISTS idx_chat_messages_platform_channel_timestamp
    ON chat_messages (platform, channel, timestamp);