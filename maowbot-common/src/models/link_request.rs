use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LinkRequest {
    pub link_request_id: Uuid,
    pub requesting_user_id: Uuid,
    pub target_platform: Option<String>,
    pub target_platform_user_id: Option<String>,
    pub link_code: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LinkRequest {
    pub fn new(
        requesting_user_id: Uuid,
        target_platform: Option<&str>,
        target_platform_user_id: Option<&str>,
        link_code: Option<&str>,
    ) -> Self {
        let now = Utc::now();
        Self {
            link_request_id: Uuid::new_v4(),
            requesting_user_id,
            target_platform: target_platform.map(|s| s.to_string()),
            target_platform_user_id: target_platform_user_id.map(|s| s.to_string()),
            link_code: link_code.map(|s| s.to_string()),
            status: "pending".to_string(),
            created_at: now,
            updated_at: now,
        }
    }
}