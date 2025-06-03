// High-level command handlers that can be used by both TUI and GUI
// These return structured data instead of formatted strings

pub mod user;
pub mod platform;
pub mod twitch;

/// Result type that can include both data and warnings
pub struct CommandResult<T> {
    pub data: T,
    pub warnings: Vec<String>,
}

impl<T> CommandResult<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            warnings: vec![],
        }
    }
    
    pub fn with_warnings(data: T, warnings: Vec<String>) -> Self {
        Self {
            data,
            warnings,
        }
    }
}

/// Common error type for command operations
#[derive(Debug)]
pub enum CommandError {
    GrpcError(String),
    NotFound(String),
    InvalidInput(String),
    DataError(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::GrpcError(msg) => write!(f, "gRPC error: {}", msg),
            CommandError::NotFound(msg) => write!(f, "Not found: {}", msg),
            CommandError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            CommandError::DataError(msg) => write!(f, "Data error: {}", msg),
        }
    }
}

impl std::error::Error for CommandError {}