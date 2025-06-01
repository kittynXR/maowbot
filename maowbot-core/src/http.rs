//! HTTP Client abstraction layer for platform integrations
//!
//! This module provides a generic interface for making HTTP requests across the application.
//! The abstraction serves multiple purposes:
//!
//! - Enables mocking of HTTP calls during testing without requiring real network requests
//! - Provides a consistent interface for error handling across different platform integrations
//! - Allows for future flexibility in HTTP client implementation without affecting platform code
//!
//! The default implementation wraps reqwest, but the trait-based design allows for
//! alternative implementations if needed.
//!
//! # Example Usage:
//! ``
//! use crate::http::{HttpClient, DefaultHttpClient};
//!
//! struct PlatformAPI {
//!     client: Box<dyn HttpClient<Error = Error>>,
//! }
//!
//! // In production code
//! let api = PlatformAPI {
//!     client: Box::new(DefaultHttpClient::new())
//! };
//!
//! // In tests, can use a mock implementation
//! let api = PlatformAPI {
//!     client: Box::new(MockHttpClient::new())
//! };
//! ``

use async_trait::async_trait;
use reqwest;
use std::collections::HashMap;
use crate::Error;

/// A generic trait for making HTTP requests.
#[async_trait]
pub trait HttpClient: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn post(&self, url: String, body: String) -> Result<String, Self::Error>;
    async fn get(&self, url: String, headers: HashMap<String, String>) -> Result<String, Self::Error>;
}

#[derive(Clone)]
pub struct DefaultHttpClient {
    client: reqwest::Client,
}

impl DefaultHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl HttpClient for DefaultHttpClient {
    type Error = Error;

    async fn post(&self, url: String, body: String) -> Result<String, Self::Error> {
        let response = self.client
            .post(&url)
            .body(body)
            .send()
            .await?
            .text()
            .await?;
        Ok(response)
    }

    async fn get(&self, url: String, headers: HashMap<String, String>) -> Result<String, Self::Error> {
        let mut request = self.client.get(&url);
        for (key, value) in headers {
            request = request.header(&key, value);
        }
        let response = request
            .send()
            .await?
            .text()
            .await?;
        Ok(response)
    }
}