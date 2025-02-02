-- migrations/20241216000001_add_platform_identities.sql (Postgres version)

CREATE TABLE IF NOT EXISTS platform_identities (
    platform_identity_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    platform_user_id TEXT NOT NULL,
    platform_username TEXT NOT NULL,
    platform_display_name TEXT,
    platform_roles TEXT NOT NULL,  -- JSON array
    platform_data TEXT NOT NULL,   -- JSON object
    created_at BIGINT NOT NULL,
    last_updated BIGINT NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id),
    UNIQUE (platform, platform_user_id)
);

CREATE INDEX IF NOT EXISTS idx_platform_identities_user
    ON platform_identities (user_id);

CREATE INDEX IF NOT EXISTS idx_platform_identities_platform
    ON platform_identities (platform, platform_user_id);
