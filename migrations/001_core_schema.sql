-- 001_core_schema.sql
-- Core database schema for MaowBot
-- Consolidated from original migrations 001, parts of 005, 009

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Drop existing tables to ensure clean slate
DROP TABLE IF EXISTS 
    user_audit_log,
    platform_identities,
    platform_credentials,
    user_analysis,
    users,
    platform_config,
    bot_config,
    autostart_config
CASCADE;

---------------------------------------------------------------------------
-- CORE USER SYSTEM
---------------------------------------------------------------------------

-- Users table - Core user identity
CREATE TABLE users (
    user_id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    global_username TEXT UNIQUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active       BOOLEAN NOT NULL DEFAULT true,
    
    -- Indexes for performance
    CONSTRAINT users_global_username_check CHECK (global_username ~ '^[a-zA-Z0-9_-]+$')
);

CREATE INDEX idx_users_last_seen ON users(last_seen);
CREATE INDEX idx_users_is_active ON users(is_active);

COMMENT ON TABLE users IS 'Core user identity table - represents unique individuals across all platforms';
COMMENT ON COLUMN users.global_username IS 'Unique username across all platforms, alphanumeric + underscore/hyphen only';

-- User analysis scores
CREATE TABLE user_analysis (
    user_analysis_id      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id               UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    spam_score            FLOAT4 NOT NULL DEFAULT 0.0,
    intelligibility_score FLOAT4 NOT NULL DEFAULT 0.5,
    quality_score         FLOAT4 NOT NULL DEFAULT 0.5,
    horni_score           FLOAT4 NOT NULL DEFAULT 0.0,
    ai_notes              TEXT,
    moderator_notes       TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure one analysis per user
    CONSTRAINT user_analysis_user_id_unique UNIQUE(user_id),
    -- Validate score ranges
    CONSTRAINT spam_score_range CHECK (spam_score >= 0 AND spam_score <= 1),
    CONSTRAINT intelligibility_score_range CHECK (intelligibility_score >= 0 AND intelligibility_score <= 1),
    CONSTRAINT quality_score_range CHECK (quality_score >= 0 AND quality_score <= 1),
    CONSTRAINT horni_score_range CHECK (horni_score >= 0 AND horni_score <= 1)
);

CREATE INDEX idx_user_analysis_scores ON user_analysis(spam_score, quality_score);

-- User audit log for tracking changes
CREATE TABLE user_audit_log (
    audit_id      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id       UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    action        TEXT NOT NULL,
    field_name    TEXT,
    old_value     TEXT,
    new_value     TEXT,
    performed_by  UUID REFERENCES users(user_id),
    performed_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata      JSONB
);

CREATE INDEX idx_user_audit_log_user_id ON user_audit_log(user_id);
CREATE INDEX idx_user_audit_log_performed_at ON user_audit_log(performed_at);

---------------------------------------------------------------------------
-- PLATFORM IDENTITY SYSTEM
---------------------------------------------------------------------------

-- Platform identities - Links users to their platform accounts
CREATE TABLE platform_identities (
    platform_identity_id  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id               UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    platform              TEXT NOT NULL,
    platform_user_id      TEXT NOT NULL,
    platform_username     TEXT NOT NULL,
    platform_display_name TEXT,
    platform_roles        JSONB NOT NULL DEFAULT '[]'::jsonb,
    platform_data         JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_updated          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Ensure unique platform identity
    CONSTRAINT platform_identities_unique UNIQUE(platform, platform_user_id),
    -- Validate platform values
    CONSTRAINT platform_check CHECK (platform IN ('twitch', 'twitch-irc', 'twitch-eventsub', 'discord', 'vrchat', 'obs'))
);

CREATE INDEX idx_platform_identities_user_id ON platform_identities(user_id);
CREATE INDEX idx_platform_identities_platform ON platform_identities(platform);

-- Platform credentials - OAuth tokens and authentication
CREATE TABLE platform_credentials (
    credential_id   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform        TEXT NOT NULL,
    platform_id     TEXT,
    credential_type TEXT NOT NULL,
    user_id         UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    user_name       TEXT NOT NULL,
    primary_token   TEXT NOT NULL, -- Should be encrypted in application
    refresh_token   TEXT,          -- Should be encrypted in application
    additional_data JSONB,
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_broadcaster  BOOLEAN NOT NULL DEFAULT false,
    is_teammate     BOOLEAN NOT NULL DEFAULT false,
    is_bot          BOOLEAN NOT NULL DEFAULT false,
    
    -- Validate platform values
    CONSTRAINT platform_cred_check CHECK (platform IN ('twitch', 'twitch-irc', 'twitch-eventsub', 'discord', 'vrchat', 'obs')),
    -- Validate credential types
    CONSTRAINT credential_type_check CHECK (credential_type IN ('oauth', 'api_key', 'password', 'token'))
);

CREATE INDEX idx_platform_credentials_platform ON platform_credentials(platform);
CREATE INDEX idx_platform_credentials_user_id ON platform_credentials(user_id);
CREATE INDEX idx_platform_credentials_expires ON platform_credentials(expires_at) WHERE expires_at IS NOT NULL;

-- Platform configuration (API keys, client IDs, etc)
CREATE TABLE platform_config (
    platform_config_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform           TEXT NOT NULL UNIQUE,
    client_id          TEXT,
    client_secret      TEXT, -- Should be encrypted in application
    webhook_secret     TEXT, -- For platforms that use webhooks
    api_endpoint       TEXT, -- Custom API endpoints
    additional_config  JSONB,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT platform_config_check CHECK (platform IN ('twitch', 'discord', 'vrchat', 'obs'))
);

---------------------------------------------------------------------------
-- BOT CONFIGURATION
---------------------------------------------------------------------------

-- Generic bot configuration storage
CREATE TABLE bot_config (
    config_key     TEXT NOT NULL,
    config_value   TEXT NOT NULL,
    config_type    TEXT DEFAULT 'string',
    config_meta    JSONB,
    description    TEXT,
    is_sensitive   BOOLEAN DEFAULT false, -- Flag for values that should be encrypted
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (config_key),
    CONSTRAINT config_type_check CHECK (config_type IN ('string', 'number', 'boolean', 'json', 'encrypted'))
);

CREATE INDEX idx_bot_config_type ON bot_config(config_type);

-- Autostart configuration for plugins and services
CREATE TABLE autostart_config (
    autostart_id   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    component_type TEXT NOT NULL, -- 'plugin', 'service', 'platform'
    component_name TEXT NOT NULL,
    enabled        BOOLEAN NOT NULL DEFAULT true,
    start_order    INT NOT NULL DEFAULT 100,
    start_delay_ms INT DEFAULT 0,
    restart_policy TEXT DEFAULT 'on-failure', -- 'always', 'on-failure', 'never'
    max_restarts   INT DEFAULT 3,
    config_data    JSONB,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT autostart_unique UNIQUE(component_type, component_name),
    CONSTRAINT restart_policy_check CHECK (restart_policy IN ('always', 'on-failure', 'never'))
);

CREATE INDEX idx_autostart_enabled ON autostart_config(enabled);
CREATE INDEX idx_autostart_order ON autostart_config(start_order);

-- Legacy autostart table for platform connections
CREATE TABLE autostart (
    id SERIAL PRIMARY KEY,
    platform VARCHAR(50) NOT NULL,
    account_name VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(platform, account_name)
);

CREATE INDEX idx_autostart_platform_enabled ON autostart(enabled) WHERE enabled = true;

---------------------------------------------------------------------------
-- ANALYTICS AND TRACKING
---------------------------------------------------------------------------

-- Analytics events table with proper partitioning
CREATE TABLE analytics_events (
    event_id        UUID NOT NULL DEFAULT uuid_generate_v4(),
    event_type      TEXT NOT NULL,
    event_category  TEXT NOT NULL,
    event_source    TEXT,
    event_data      JSONB NOT NULL DEFAULT '{}'::jsonb,
    user_id         UUID REFERENCES users(user_id) ON DELETE SET NULL,
    platform        TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Fixed: PRIMARY KEY must include partition column
    PRIMARY KEY (event_id, created_at),
    CONSTRAINT event_type_not_empty CHECK (event_type != '')
) PARTITION BY RANGE (created_at);

CREATE INDEX idx_analytics_events_type ON analytics_events(event_type);
CREATE INDEX idx_analytics_events_category ON analytics_events(event_category);
CREATE INDEX idx_analytics_events_created ON analytics_events(created_at);
CREATE INDEX idx_analytics_events_user ON analytics_events(user_id) WHERE user_id IS NOT NULL;

-- Chat messages table with proper partitioning
CREATE TABLE chat_messages (
    message_id      UUID NOT NULL DEFAULT uuid_generate_v4(),
    platform        TEXT NOT NULL,
    channel         TEXT NOT NULL,
    user_id         UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    message_text    TEXT NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    metadata        JSONB,
    PRIMARY KEY (message_id, timestamp)
) PARTITION BY RANGE (timestamp);

-- Use BRIN index for better performance on time-series data
CREATE INDEX idx_chat_messages_timestamp_brin ON chat_messages USING BRIN(timestamp);
CREATE INDEX idx_chat_messages_channel ON chat_messages(platform, channel, timestamp DESC);
CREATE INDEX idx_chat_messages_user ON chat_messages(user_id, timestamp DESC);

-- Chat logging configuration per channel
CREATE TABLE chat_logging_config (
    config_id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform                TEXT NOT NULL,
    channel                 TEXT NOT NULL,
    is_enabled              BOOLEAN NOT NULL DEFAULT true,
    retention_days          INT NOT NULL DEFAULT 30,
    sampling_rate           FLOAT NOT NULL DEFAULT 1.0, -- 1.0 = 100%, 0.1 = 10%
    pre_drop_pipeline_id    UUID, -- Will reference event_pipelines after it's created
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_channel_config UNIQUE(platform, channel),
    CONSTRAINT sampling_rate_check CHECK (sampling_rate > 0 AND sampling_rate <= 1),
    CONSTRAINT retention_days_check CHECK (retention_days > 0)
);

CREATE INDEX idx_chat_logging_config_enabled ON chat_logging_config(is_enabled);

-- Chat sessions tracking
CREATE TABLE chat_sessions (
    session_id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform                TEXT NOT NULL,
    channel                 TEXT NOT NULL,
    user_id                 UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    joined_at               TIMESTAMPTZ NOT NULL,
    left_at                 TIMESTAMPTZ,
    session_duration_seconds BIGINT,
    message_count           INT NOT NULL DEFAULT 0,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_chat_sessions_user ON chat_sessions(user_id);
CREATE INDEX idx_chat_sessions_active ON chat_sessions(left_at) WHERE left_at IS NULL;

-- Bot events (for non-chat events)
CREATE TABLE bot_events (
    event_id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type      TEXT NOT NULL,
    event_timestamp TIMESTAMPTZ NOT NULL,
    data            JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bot_events_type ON bot_events(event_type);
CREATE INDEX idx_bot_events_timestamp ON bot_events(event_timestamp DESC);

-- Daily statistics
CREATE TABLE daily_stats (
    date                DATE PRIMARY KEY,
    total_messages      BIGINT NOT NULL DEFAULT 0,
    total_chat_visits   BIGINT NOT NULL DEFAULT 0,
    unique_users        INT NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- User analysis history (for tracking changes over time)
CREATE TABLE user_analysis_history (
    user_analysis_history_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id                  UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    year_month               TEXT NOT NULL, -- Format: YYYY-MM
    spam_score               FLOAT4 NOT NULL DEFAULT 0.0,
    intelligibility_score    FLOAT4 NOT NULL DEFAULT 0.5,
    quality_score            FLOAT4 NOT NULL DEFAULT 0.5,
    horni_score              FLOAT4 NOT NULL DEFAULT 0.0,
    ai_notes                 TEXT,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT user_analysis_history_unique UNIQUE(user_id, year_month)
);

CREATE INDEX idx_user_analysis_history_user ON user_analysis_history(user_id);
CREATE INDEX idx_user_analysis_history_month ON user_analysis_history(year_month);

---------------------------------------------------------------------------
-- HELPER FUNCTIONS
---------------------------------------------------------------------------

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply updated_at triggers to relevant tables
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_user_analysis_updated_at BEFORE UPDATE ON user_analysis
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_platform_identities_updated_at BEFORE UPDATE ON platform_identities
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_platform_credentials_updated_at BEFORE UPDATE ON platform_credentials
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_platform_config_updated_at BEFORE UPDATE ON platform_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_bot_config_updated_at BEFORE UPDATE ON bot_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_autostart_config_updated_at BEFORE UPDATE ON autostart_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_chat_logging_config_updated_at BEFORE UPDATE ON chat_logging_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

---------------------------------------------------------------------------
-- INITIAL PARTITIONS
---------------------------------------------------------------------------

-- Create initial partitions for current and next month
DO $$
DECLARE
    current_month DATE := DATE_TRUNC('month', NOW());
    next_month DATE := DATE_TRUNC('month', NOW() + INTERVAL '1 month');
    next_next_month DATE := DATE_TRUNC('month', NOW() + INTERVAL '2 months');
BEGIN
    -- For analytics_events
    EXECUTE format('CREATE TABLE IF NOT EXISTS analytics_events_%s PARTITION OF analytics_events FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(current_month, 'YYYYMM'), current_month, next_month);
    
    EXECUTE format('CREATE TABLE IF NOT EXISTS analytics_events_%s PARTITION OF analytics_events FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(next_month, 'YYYYMM'), next_month, next_next_month);
    
    -- For chat_messages
    EXECUTE format('CREATE TABLE IF NOT EXISTS chat_messages_%s PARTITION OF chat_messages FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(current_month, 'YYYYMM'), current_month, next_month);
    
    EXECUTE format('CREATE TABLE IF NOT EXISTS chat_messages_%s PARTITION OF chat_messages FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(next_month, 'YYYYMM'), next_month, next_next_month);
END$$;