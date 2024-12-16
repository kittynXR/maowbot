-- Platform identities table
CREATE TABLE IF NOT EXISTS platform_identities (
    platform_identity_id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    platform_user_id TEXT NOT NULL,
    platform_username TEXT NOT NULL,
    platform_display_name TEXT,  -- This can be nullable
    platform_roles TEXT NOT NULL,  -- JSON array of roles
    platform_data TEXT NOT NULL,  -- JSON for platform-specific data
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_updated TIMESTAMP NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(user_id),
    UNIQUE (platform, platform_user_id)
);

-- Index for faster lookups
CREATE INDEX idx_platform_identities_user ON platform_identities(user_id);
CREATE INDEX idx_platform_identities_platform ON platform_identities(platform, platform_user_id);