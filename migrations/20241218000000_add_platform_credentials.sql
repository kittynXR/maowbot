-- add platform credentials

CREATE TABLE platform_credentials (
    credential_id TEXT NOT NULL PRIMARY KEY,
    platform TEXT NOT NULL,
    credential_type TEXT NOT NULL, -- 'oauth', 'token', 'apikey', 'vc', etc
    user_id TEXT NOT NULL, -- reference to the bot operator/broadcaster
    primary_token TEXT NOT NULL, -- encrypted main token
    refresh_token TEXT, -- encrypted refresh token if applicable
    additional_data TEXT, -- encrypted JSON for extra auth data
    expires_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(user_id),
    UNIQUE (platform, user_id)
);

CREATE INDEX idx_platform_credentials_user ON platform_credentials(user_id, platform);