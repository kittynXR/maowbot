-- migrations/20250128000000_add_user_analysis_history.sql (Postgres)

CREATE TABLE IF NOT EXISTS user_analysis_history (
    user_analysis_history_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    year_month TEXT NOT NULL,
    spam_score REAL NOT NULL DEFAULT 0,
    intelligibility_score REAL NOT NULL DEFAULT 0,
    quality_score REAL NOT NULL DEFAULT 0,
    horni_score REAL NOT NULL DEFAULT 0,
    ai_notes TEXT,
    created_at BIGINT NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_analysis_history_user_month
    ON user_analysis_history (user_id, year_month);

CREATE TABLE IF NOT EXISTS maintenance_state (
    state_key TEXT PRIMARY KEY,
    state_value TEXT
);
