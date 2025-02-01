CREATE TABLE IF NOT EXISTS users (
    user_id TEXT NOT NULL PRIMARY KEY,
    global_username TEXT,
    created_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);
