use serde::Deserialize;

/// "channel.update" event
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelUpdate {
    pub broadcaster_user_id: String,
    pub broadcaster_user_login: String,
    pub broadcaster_user_name: String,
    pub title: String,
    pub language: String,
    pub category_id: String,
    pub category_name: String,
    #[serde(default)]
    pub content_classification_labels: Vec<String>,
}
