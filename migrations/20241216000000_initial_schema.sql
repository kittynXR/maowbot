-- maowbot-core/migrations/0001_init.sql
-- Completely rewritten for UUID-based primary keys.

-- Enable the uuid-ossp extension (for uuid_generate_v4, if on PostgreSQL).
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

---------------------------------------------------------------------------
-- users
---------------------------------------------------------------------------
DROP TABLE IF EXISTS users CASCADE;
CREATE TABLE users (
    user_id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    global_username TEXT,
    created_at      TIMESTAMPTZ NOT NULL,
    last_seen       TIMESTAMPTZ NOT NULL,
    is_active       BOOLEAN      NOT NULL
);

---------------------------------------------------------------------------
-- user_analysis
---------------------------------------------------------------------------
DROP TABLE IF EXISTS user_analysis CASCADE;
CREATE TABLE user_analysis (
    user_analysis_id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id              UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    spam_score           FLOAT4 NOT NULL,
    intelligibility_score FLOAT4 NOT NULL,
    quality_score        FLOAT4 NOT NULL,
    horni_score          FLOAT4 NOT NULL,
    ai_notes             TEXT,
    moderator_notes      TEXT,
    created_at           TIMESTAMPTZ NOT NULL,
    updated_at           TIMESTAMPTZ NOT NULL
);

---------------------------------------------------------------------------
-- platform_identities
---------------------------------------------------------------------------
DROP TABLE IF EXISTS platform_identities CASCADE;
CREATE TABLE platform_identities (
    platform_identity_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id              UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    platform             TEXT NOT NULL,
    platform_user_id     TEXT NOT NULL,
    platform_username    TEXT NOT NULL,
    platform_display_name TEXT,
    platform_roles       JSONB NOT NULL,
    platform_data        JSONB NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL,
    last_updated         TIMESTAMPTZ NOT NULL
);

---------------------------------------------------------------------------
-- platform_credentials
---------------------------------------------------------------------------
DROP TABLE IF EXISTS platform_credentials CASCADE;
CREATE TABLE platform_credentials (
    credential_id   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform        TEXT NOT NULL,
    platform_id     TEXT,
    credential_type TEXT NOT NULL,
    user_id         UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    user_name       TEXT NOT NULL,
    primary_token   TEXT NOT NULL,
    refresh_token   TEXT,
    additional_data TEXT,
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL,
    is_bot          BOOLEAN NOT NULL
);

-- Unique index if you want to ensure only one credential per (platform,user_id).
-- (Remove or edit if you want multiple credentials per platform.)
CREATE UNIQUE INDEX ON platform_credentials (platform, user_id);

---------------------------------------------------------------------------
-- platform_config
---------------------------------------------------------------------------
DROP TABLE IF EXISTS platform_config CASCADE;
CREATE TABLE platform_config (
    platform_config_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform           TEXT NOT NULL,
    client_id          TEXT,
    client_secret      TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- bot_config
---------------------------------------------------------------------------
DROP TABLE IF EXISTS bot_config CASCADE;
CREATE TABLE bot_config (
    config_key   TEXT PRIMARY KEY,
    config_value TEXT
);

---------------------------------------------------------------------------
-- link_requests
---------------------------------------------------------------------------
DROP TABLE IF EXISTS link_requests CASCADE;
CREATE TABLE link_requests (
    link_request_id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    requesting_user_id     UUID NOT NULL,
    target_platform        TEXT,
    target_platform_user_id TEXT,
    link_code             TEXT,
    status                TEXT NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL,
    updated_at            TIMESTAMPTZ NOT NULL
);

---------------------------------------------------------------------------
-- user_audit_log
---------------------------------------------------------------------------
DROP TABLE IF EXISTS user_audit_log CASCADE;
CREATE TABLE user_audit_log (
    audit_id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id      UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    event_type   TEXT NOT NULL,
    old_value    TEXT,
    new_value    TEXT,
    changed_by   TEXT,
    timestamp    TIMESTAMPTZ NOT NULL,
    metadata     TEXT
);

---------------------------------------------------------------------------
-- chat_messages (example partitioned table)
---------------------------------------------------------------------------
CREATE TABLE chat_messages (
    message_id UUID   NOT NULL,
    platform   TEXT   NOT NULL,
    channel    TEXT   NOT NULL,
    user_id    UUID   NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    message_text TEXT NOT NULL,
    timestamp    TIMESTAMPTZ NOT NULL,
    metadata     TEXT,
    PRIMARY KEY (message_id, timestamp)
) PARTITION BY RANGE (timestamp);

---------------------------------------------------------------------------
-- daily_stats
---------------------------------------------------------------------------
DROP TABLE IF EXISTS daily_stats CASCADE;
CREATE TABLE daily_stats (
    date                  DATE PRIMARY KEY,
    total_messages        BIGINT NOT NULL DEFAULT 0,
    total_chat_visits     BIGINT NOT NULL DEFAULT 0
);

---------------------------------------------------------------------------
-- chat_sessions
---------------------------------------------------------------------------
DROP TABLE IF EXISTS chat_sessions CASCADE;
CREATE TABLE chat_sessions (
    session_id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform                 TEXT NOT NULL,
    channel                  TEXT NOT NULL,
    user_id                  UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    joined_at                TIMESTAMPTZ NOT NULL,
    left_at                  TIMESTAMPTZ,
    session_duration_seconds BIGINT
);

---------------------------------------------------------------------------
-- bot_events
---------------------------------------------------------------------------
DROP TABLE IF EXISTS bot_events CASCADE;
CREATE TABLE bot_events (
    event_id       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type     TEXT NOT NULL,
    event_timestamp TIMESTAMPTZ NOT NULL,
    data           TEXT
);

---------------------------------------------------------------------------
-- user_analysis_history (if you use it)
---------------------------------------------------------------------------
DROP TABLE IF EXISTS user_analysis_history CASCADE;
CREATE TABLE user_analysis_history (
    user_analysis_history_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id                  UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    year_month               TEXT NOT NULL,
    spam_score               FLOAT4 NOT NULL,
    intelligibility_score    FLOAT4 NOT NULL,
    quality_score            FLOAT4 NOT NULL,
    horni_score              FLOAT4 NOT NULL,
    ai_notes                 TEXT,
    created_at               TIMESTAMPTZ NOT NULL
);
