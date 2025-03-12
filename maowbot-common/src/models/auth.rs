use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tokio::sync::{oneshot, Mutex};
pub use crate::models::platform::Platform;
use crate::models::user::User;

#[derive(Debug, Clone)]
pub enum AuthenticationPrompt {
    Browser { url: String },
    Code { message: String },
    ApiKey { message: String },
    MultipleKeys { fields: Vec<String>, messages: Vec<String> },
    TwoFactor { message: String },
    None,
}

#[derive(Debug)]
pub enum AuthenticationResponse {
    Code(String),
    ApiKey(String),
    MultipleKeys(std::collections::HashMap<String, String>),
    TwoFactor(String),
    None,
}

/// Structure to hold the final result from the OAuth callback.
#[derive(Debug, Clone)]
pub struct CallbackResult {
    pub code: String,
    pub state: Option<String>,
}

/// Query string we expect from e.g. Twitch: ?code=xxx&state=...
#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Shared state for the Axum callback route.
#[derive(Clone)]
pub struct CallbackServerState {
    /// Once we receive a code, we send it through `done_tx`.
    pub done_tx: Arc<Mutex<Option<oneshot::Sender<CallbackResult>>>>,
}

#[derive(Debug, Clone)]
struct CachedUser {
    user: User,
    last_access: DateTime<Utc>,
}