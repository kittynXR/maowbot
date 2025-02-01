-- 1) Create a table for user- or mod-initiated link requests
CREATE TABLE IF NOT EXISTS link_requests (
    link_request_id TEXT NOT NULL PRIMARY KEY,
    requesting_user_id TEXT NOT NULL,      -- references users(user_id)
    target_platform TEXT,                  -- e.g. "twitch", "discord", "vrchat"
    target_platform_user_id TEXT,          -- e.g. "twitch_12345"
    link_code TEXT,                        -- ephemeral code for user to confirm
    status TEXT NOT NULL DEFAULT 'pending',-- "pending", "approved", "denied", etc
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (requesting_user_id) REFERENCES users(user_id)
);

-- 2) Create a table for auditing name changes, link approvals, merges, etc.
CREATE TABLE IF NOT EXISTS user_audit_log (
    audit_id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL,         -- the user who was changed
    event_type TEXT NOT NULL,      -- e.g. "name_change", "link_approved"
    old_value TEXT,                -- optional, e.g. old username or old link
    new_value TEXT,                -- optional, e.g. new username or new link
    changed_by TEXT,               -- user_id of the mod or the user themselves
    timestamp INTEGER NOT NULL,
    metadata TEXT,                 -- optional JSON for extra context
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);
