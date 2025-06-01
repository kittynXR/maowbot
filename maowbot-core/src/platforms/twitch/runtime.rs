// File: src/platforms/twitch_helix/runtime.rs

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use async_trait::async_trait;
use http::{Request, Response};
use twitch_api::{HelixClient, HttpClient};
use reqwest::Client as ReqwestClient;

use crate::Error;
use maowbot_common::models::platform::PlatformCredential;
use maowbot_common::traits::platform_traits::{ConnectionStatus, PlatformAuth, PlatformIntegration};
use twitch_api::client::Bytes as TwitchBytes;

#[derive(Debug)]
pub struct MyErrorWrapper(Box<dyn std::error::Error + Send + Sync + 'static>);

impl std::fmt::Display for MyErrorWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for MyErrorWrapper {}

impl From<Box<dyn std::error::Error + Send + Sync + 'static>> for MyErrorWrapper {
    fn from(e: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
        MyErrorWrapper(e)
    }
}

/// A wrapper around Arc<reqwest::Client> that implements twitch_api::HttpClient.
/// This makes it possible to use reqwest as the HTTP client for Helix calls.
#[derive(Clone)]
pub struct MyReqwestHTTPClient {
    pub inner: Arc<ReqwestClient>,
}

impl HttpClient for MyReqwestHTTPClient {
    // Use our newtype as the error type.
    type Error = MyErrorWrapper;

    fn req<'a>(
        &'a self,
        request: Request<TwitchBytes>,
    ) -> Pin<Box<dyn Future<Output = Result<Response<TwitchBytes>, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            // Split request into parts.
            let (parts, body) = request.into_parts();
            // Convert the body (which is TwitchBytes) to standard bytes.
            let body_bytes = bytes::Bytes::copy_from_slice(body.as_ref());

            // Build the reqwest request from the http::Request parts.
            let mut reqwest_builder = self.inner.request(parts.method, parts.uri.to_string());
            // Copy all headers.
            for (name, value) in parts.headers {
                if let Some(name_key) = name {
                    reqwest_builder = reqwest_builder.header(name_key, value);
                }
            }
            reqwest_builder = reqwest_builder.body(body_bytes);

            // Send the request using reqwest.
            let reqwest_response = reqwest_builder.send().await.map_err(|err| {
                Box::new(err) as Box<dyn std::error::Error + Send + Sync + 'static>
            })?;

            // Build an http::Response.
            let status = reqwest_response.status();
            let mut builder = Response::builder().status(status);
            for (k, v) in reqwest_response.headers().iter() {
                builder = builder.header(k, v);
            }
            let resp_bytes = reqwest_response.bytes().await.map_err(|err| {
                Box::new(err) as Box<dyn std::error::Error + Send + Sync + 'static>
            })?;
            let resp_body = TwitchBytes::copy_from_slice(&resp_bytes);
            let response = builder.body(resp_body).map_err(|http_err| {
                // Convert the http::Error to our boxed error.
                Box::<dyn std::error::Error + Send + Sync + 'static>::from(http_err.to_string())
            })?;

            Ok(response)
        })
    }
}

/// The primary Twitch platform struct.
/// - We store an Option<HelixClient<'static, Arc<ReqwestClient>>> so we can make Helix calls.
/// - This requires "http_impl_reqwest" feature in twitch_api = "0.7"
pub struct TwitchPlatform {
    pub credentials: Option<PlatformCredential>,
    pub connection_status: ConnectionStatus,

    /// Owned Helix client, `'static` lifetime,
    /// Arc<ReqwestClient> so we only have one underlying client.
    pub client: Option<HelixClient<'static, Arc<ReqwestClient>>>,
}

impl TwitchPlatform {
    /// Example constructor that builds a reqwest Client + HelixClient
    pub fn new() -> Self {
        // 1) Build a reqwest client, wrap it in Arc
        let arc_client = Arc::new(ReqwestClient::new());
        // 2) Build a HelixClient from that Arc
        let helix_client = HelixClient::with_client(arc_client);

        Self {
            credentials: None,
            connection_status: ConnectionStatus::Disconnected,
            client: Some(helix_client),
        }
    }
}

/// Example chat message event
pub struct TwitchMessageEvent {
    pub channel: String,
    pub user_id: String,
    pub display_name: String,
    pub text: String,
}

#[async_trait]
impl PlatformAuth for TwitchPlatform {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // E.g. store OAuth creds
        Ok(())
    }

    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // Refresh the token if needed
        Ok(())
    }

    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // Clear out credential data
        self.credentials = None;
        // Drop the client if you want to fully "disconnect"
        self.client = None;
        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool, Error> {
        // If we have a credentials object, consider ourselves authenticated
        Ok(self.credentials.is_some())
    }
}

#[async_trait]
impl PlatformIntegration for TwitchPlatform {
    async fn connect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Connected;
        // Real logic might connect to Twitch IRC or EventSub
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connection_status = ConnectionStatus::Disconnected;
        // Cleanly shut down
        Ok(())
    }

    async fn send_message(&self, _channel: &str, _message: &str) -> Result<(), Error> {
        // e.g. Helix or IRC call
        Ok(())
    }

    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.connection_status.clone())
    }
}

impl TwitchPlatform {
    /// Stub that might poll your IRC or event queue
    pub async fn next_message_event(&mut self) -> Option<TwitchMessageEvent> {
        None
    }
}