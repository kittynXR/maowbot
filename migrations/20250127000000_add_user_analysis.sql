-- migrations/20250127000000_add_user_analysis.sql (Postgres)

CREATE TABLE IF NOT EXISTS user_analysis (
    user_analysis_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    spam_score REAL NOT NULL DEFAULT 0,
    intelligibility_score REAL NOT NULL DEFAULT 0,
    quality_score REAL NOT NULL DEFAULT 0,
    horni_score REAL NOT NULL DEFAULT 0,
    ai_notes TEXT,
    moderator_notes TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_analysis_user
    ON user_analysis (user_id);
