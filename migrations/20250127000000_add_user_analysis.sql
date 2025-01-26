-- migrations/20250121000002_add_user_analysis.sql

CREATE TABLE IF NOT EXISTS user_analysis (
    user_analysis_id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL,           -- references users(user_id)
    spam_score REAL NOT NULL DEFAULT 0,
    intelligibility_score REAL NOT NULL DEFAULT 0,
    quality_score REAL NOT NULL DEFAULT 0,
    horni_score REAL NOT NULL DEFAULT 0,
    ai_notes TEXT,                   -- freeform AI metadata
    moderator_notes TEXT,            -- mod's hand-written notes
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_analysis_user
    ON user_analysis (user_id);
