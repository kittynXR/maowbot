// ================================================================
// File: maowbot-common/src/error.rs
// ================================================================

use thiserror::Error;
use rcgen;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Not found error: {0}")]
    NotFound(String),

    // Existing variants:
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Invalid credential type: {0}")]
    InvalidCredentialType(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Key derivation error: {0}")]
    KeyDerivation(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    // New variant for InvalidUri errors:
    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    // New variants for errors coming from various parts of main:
    #[error("Address parse error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),

    #[error("Tonic transport error: {0}")]
    Tonic(#[from] tonic::transport::Error),

    #[error("MPSC send error: {0}")]
    MpscSend(#[from] tokio::sync::mpsc::error::SendError<maowbot_proto::plugs::PluginStreamRequest>),

    #[error("gRPC status error: {0}")]
    GrpcStatus(#[from] tonic::Status),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Timeout error: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("Library loading error: {0}")]
    LibLoading(#[from] libloading::Error),

    #[error("Keyring error: {0}")]
    Keyring(String),

    #[error("Uuid error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Event bus error: {0}")]
    EventBus(String),

    // <-- ADD THIS NEW VARIANT FOR `rcgen::Error` -->
    #[error("Certificate generation error: {0}")]
    Certificate(#[from] rcgen::Error),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Parse(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Parse(s.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        // You can choose an appropriate variant.
        // For example, here we wrap any anyhow error into our Parse variant.
        Error::Parse(e.to_string())
    }
}

impl From<chrono::format::ParseError> for Error {
    fn from(err: chrono::format::ParseError) -> Self {
        Error::Parse(err.to_string())
    }
}

impl From<keyring::Error> for Error {
    fn from(err: keyring::Error) -> Self {
        Error::Auth(format!("keyring error: {}", err))
    }
}

// New conversion for http::uri::InvalidUri:
impl From<http::uri::InvalidUri> for Error {
    fn from(err: http::uri::InvalidUri) -> Self {
        Error::InvalidUri(err.to_string())
    }
}
