use thiserror::Error;

#[derive(Error, Debug)]
pub enum ObsError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    
    #[error("Scene not found: {0}")]
    SceneNotFound(String),
    
    #[error("Source not found: {0}")]
    SourceNotFound(String),
    
    #[error("Invalid instance number: {0}")]
    InvalidInstance(u32),
    
    #[error("Instance not connected: {0}")]
    InstanceNotConnected(u32),
    
    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ObsError>;