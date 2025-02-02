-- migrations/20241216000000_initial_schema.sql  (Postgres version)

CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    global_username TEXT,
    created_at BIGINT NOT NULL,
    last_seen BIGINT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);
