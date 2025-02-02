-- migrations/20250121000001_add_username_and_link_tables.sql (Postgres)

CREATE TABLE IF NOT EXISTS link_requests (
    link_request_id TEXT PRIMARY KEY,
    requesting_user_id TEXT NOT NULL,
    target_platform TEXT,
    target_platform_user_id TEXT,
    link_code TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT fk_requesting_user FOREIGN KEY (requesting_user_id) REFERENCES users(user_id)
);

CREATE TABLE IF NOT EXISTS user_audit_log (
    audit_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    changed_by TEXT,
    timestamp BIGINT NOT NULL,
    metadata TEXT,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id)
);