#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Authentication error: {0}")]
    Auth(String),
}