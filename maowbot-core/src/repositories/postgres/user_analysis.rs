use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::{Error, models::UserAnalysis};

#[async_trait]
pub trait UserAnalysisRepository: Send + Sync {
    async fn create_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error>;
    async fn get_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error>;
    async fn update_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error>;
}

#[derive(Clone)]
pub struct PostgresUserAnalysisRepository {
    pool: Pool<Postgres>,
}

impl PostgresUserAnalysisRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserAnalysisRepository for PostgresUserAnalysisRepository {
    async fn create_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error> {
        sqlx::query(
            r#"
            INSERT INTO user_analysis (
                user_analysis_id,
                user_id,
                spam_score,
                intelligibility_score,
                quality_score,
                horni_score,
                ai_notes,
                moderator_notes,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
            .bind(analysis.user_analysis_id)
            .bind(analysis.user_id)
            .bind(analysis.spam_score)
            .bind(analysis.intelligibility_score)
            .bind(analysis.quality_score)
            .bind(analysis.horni_score)
            .bind(&analysis.ai_notes)
            .bind(&analysis.moderator_notes)
            .bind(analysis.created_at)
            .bind(analysis.updated_at)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_analysis(&self, user_id: Uuid) -> Result<Option<UserAnalysis>, Error> {
        let row = sqlx::query(
            r#"
            SELECT user_analysis_id,
                   user_id,
                   spam_score,
                   intelligibility_score,
                   quality_score,
                   horni_score,
                   ai_notes,
                   moderator_notes,
                   created_at,
                   updated_at
            FROM user_analysis
            WHERE user_id = $1
            "#,
        )
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let ua = UserAnalysis {
                user_analysis_id: r.try_get("user_analysis_id")?,
                user_id: r.try_get("user_id")?,
                spam_score: r.try_get("spam_score")?,
                intelligibility_score: r.try_get("intelligibility_score")?,
                quality_score: r.try_get("quality_score")?,
                horni_score: r.try_get("horni_score")?,
                ai_notes: r.try_get("ai_notes")?,
                moderator_notes: r.try_get("moderator_notes")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            };
            Ok(Some(ua))
        } else {
            Ok(None)
        }
    }

    async fn update_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error> {
        // ----------------------------------------------------------------
        // NEW: Clean up the AI notes before saving, to avoid piling up
        // repeated monthly summary lines. In this example, we remove
        // any duplicate lines that begin with "=== YYYY-MM summary ===".
        // You can adapt the logic to keep only the newest line, or
        // deduplicate by exact text, etc.
        // ----------------------------------------------------------------
        let sanitized_notes = match &analysis.ai_notes {
            Some(notes) => Some(deduplicate_ai_notes(notes)),
            None => None,
        };

        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE user_analysis
            SET spam_score = $1,
                intelligibility_score = $2,
                quality_score = $3,
                horni_score = $4,
                ai_notes = $5,
                moderator_notes = $6,
                updated_at = $7
            WHERE user_analysis_id = $8
            "#,
        )
            .bind(analysis.spam_score)
            .bind(analysis.intelligibility_score)
            .bind(analysis.quality_score)
            .bind(analysis.horni_score)
            .bind(&sanitized_notes)
            .bind(&analysis.moderator_notes)
            .bind(now)
            .bind(analysis.user_analysis_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// --------------------------------------------------------------------------
/// Utility function to remove repeated monthly summaries in `ai_notes`.
///
/// For each line that starts with `"=== YYYY-MM summary ==="`, we keep
/// only one occurrence in the final output. All other lines remain unchanged.
/// Adjust to your exact desired logic (e.g. keep only the last occurrence).
/// --------------------------------------------------------------------------
fn deduplicate_ai_notes(full_text: &str) -> String {
    use std::collections::HashSet;
    let mut seen_summaries = HashSet::new();
    let mut result_lines = Vec::new();

    for line in full_text.lines() {
        // Example pattern: "=== 2025-03 summary ==="
        // We'll just check that it starts with "=== " and ends with " summary ===".
        // Then store the entire line in a set if we haven't seen it yet.
        let trimmed = line.trim();

        let is_summary = trimmed.starts_with("===") && trimmed.contains("summary ===");
        if is_summary {
            if seen_summaries.contains(trimmed) {
                // skip repeated line
                continue;
            } else {
                seen_summaries.insert(trimmed.to_string());
            }
        }

        result_lines.push(line);
    }

    result_lines.join("\n")
}