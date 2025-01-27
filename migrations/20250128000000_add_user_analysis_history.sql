-- migrations/20250128000000_add_user_analysis_history.sql

CREATE TABLE IF NOT EXISTS user_analysis_history (
    user_analysis_history_id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL,                -- references users(user_id)
    year_month TEXT NOT NULL,             -- e.g. "2025-01"
    spam_score REAL NOT NULL DEFAULT 0,
    intelligibility_score REAL NOT NULL DEFAULT 0,
    quality_score REAL NOT NULL DEFAULT 0,
    horni_score REAL NOT NULL DEFAULT 0,
    ai_notes TEXT,                        -- monthly summary from the AI
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_analysis_history_user_month
    ON user_analysis_history (user_id, year_month);

CREATE TABLE IF NOT EXISTS maintenance_state (
    state_key TEXT NOT NULL PRIMARY KEY,
    state_value TEXT
);

-- We will store "archived_until" => "YYYY-MM" in that table
-- Example row: ("archived_until", "2024-12")
