-- migrations/20241218000000_add_platform_credentials.sql (Postgres version)

CREATE TABLE IF NOT EXISTS platform_credentials (
    credential_id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    credential_type TEXT NOT NULL,
    user_id TEXT NOT NULL,
    primary_token TEXT NOT NULL,
    refresh_token TEXT,
    additional_data TEXT,
    expires_at BIGINT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT fk_userid FOREIGN KEY (user_id) REFERENCES users(user_id),
    UNIQUE (platform, user_id)
);

CREATE INDEX IF NOT EXISTS idx_platform_credentials_user
    ON platform_credentials (user_id, platform);
