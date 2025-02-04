// src/repositories/postgres/user_analysis.rs

use sqlx::{Pool, Postgres, Row};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crate::{Error, models::UserAnalysis};

#[async_trait]
pub trait UserAnalysisRepository: Send + Sync {
    async fn create_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error>;
    async fn get_analysis(&self, user_id: &str) -> Result<Option<UserAnalysis>, Error>;
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
            "#
        )
            .bind(&analysis.user_analysis_id)
            .bind(&analysis.user_id)
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

    async fn get_analysis(&self, user_id: &str) -> Result<Option<UserAnalysis>, Error> {
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
            "#
        )
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            Ok(Some(UserAnalysis {
                user_analysis_id: r.try_get("user_analysis_id")?,
                user_id: r.try_get("user_id")?,
                spam_score: r.try_get("spam_score")?,
                intelligibility_score: r.try_get("intelligibility_score")?,
                quality_score: r.try_get("quality_score")?,
                horni_score: r.try_get("horni_score")?,
                ai_notes: r.try_get("ai_notes")?,
                moderator_notes: r.try_get("moderator_notes")?,
                created_at: r.try_get::<DateTime<Utc>, _>("created_at")?,
                updated_at: r.try_get::<DateTime<Utc>, _>("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_analysis(&self, analysis: &UserAnalysis) -> Result<(), Error> {
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
            "#
        )
            .bind(analysis.spam_score)
            .bind(analysis.intelligibility_score)
            .bind(analysis.quality_score)
            .bind(analysis.horni_score)
            .bind(&analysis.ai_notes)
            .bind(&analysis.moderator_notes)
            .bind(now)
            .bind(&analysis.user_analysis_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}