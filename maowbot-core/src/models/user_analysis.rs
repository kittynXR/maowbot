// File: src/models/user_analysis.rs

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnalysis {
    pub user_analysis_id: String,
    pub user_id: String,
    pub spam_score: f32,
    pub intelligibility_score: f32,
    pub quality_score: f32,
    pub horni_score: f32,
    pub ai_notes: Option<String>,
    pub moderator_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserAnalysis {
    pub fn new(user_id: &str) -> Self {
        let now = Utc::now();
        Self {
            user_analysis_id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            spam_score: 0.0,
            intelligibility_score: 0.0,
            quality_score: 0.0,
            horni_score: 0.0,
            ai_notes: None,
            moderator_notes: None,
            created_at: now,
            updated_at: now,
        }
    }
}