// File: maowbot-core/src/auth/callback_server.rs

use std::{net::SocketAddr, sync::Arc, process::Command};
use tokio::sync::{oneshot, Mutex};
use axum::{
    Router,
    routing::get,
    extract::{Query, State},
    response::{Html},
    http::StatusCode,
};
// Use axum_server and its Handle API:
use axum_server::{Server, Handle};

use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use serde::Deserialize;
use tracing::{info, error};

use crate::{Error, repositories::postgres::app_config::AppConfigRepository};

/// Structure to hold the final result from the OAuth callback.
#[derive(Debug, Clone)]
pub struct CallbackResult {
    pub code: String,
    pub state: Option<String>,
}

/// Query string we expect from Twitch/other platforms: ?code=xxx&state=...
#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Shared state for the Axum callback route.
#[derive(Clone)]
pub struct CallbackServerState {
    /// Once we receive a code, we send it through `done_tx`.
    pub done_tx: Arc<Mutex<Option<oneshot::Sender<CallbackResult>>>>,
}

/// Start the HTTP server (Axum) on `port` and return two channels:
///
/// - `oneshot::Receiver<CallbackResult>`: The first incoming request to `/callback` that has `?code=...` will be sent through here.
/// - `oneshot::Sender<()>`:  If you send `()` through this, the server will shut down.
///
/// This server is **only** meant to be run during an OAuth flow. After a single callback, you typically shut it down.
pub async fn start_callback_server(port: u16) -> Result<(oneshot::Receiver<CallbackResult>, oneshot::Sender<()>), Error> {
    let (done_tx, done_rx) = oneshot::channel::<CallbackResult>();
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));

    let state = CallbackServerState { done_tx };

    let app = Router::new()
        .route("/callback", get(handle_callback))
        .with_state(state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
        );

    // We'll also need a shutdown signal so we can kill the server after the flow completes or on user action
    let (shutdown_send, shutdown_recv) = oneshot::channel::<()>();

    // Attempt to bind to the requested port
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("OAuth callback server listening on http://{}", addr);

    // Create a new Handle to later trigger graceful shutdown.
    let handle = Handle::new();

    // Spawn a task that waits for a shutdown signal and then calls graceful_shutdown.
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        let _ = shutdown_recv.await;
        handle_clone.graceful_shutdown(None);
    });

    let server = Server::bind(addr)
        .handle(handle)
        .serve(app.into_make_service());

    // Spawn the server in the background.
    tokio::spawn(async move {
        if let Err(e) = server.await {
            error!("Callback server error: {}", e);
        }
        info!("Callback server shut down.");
    });

    Ok((done_rx, shutdown_send))
}

/// Axum handler for GET /callback
async fn handle_callback(
    State(state): State<CallbackServerState>,
    Query(query): Query<AuthQuery>,
) -> (StatusCode, Html<String>) {
    if let Some(err) = query.error.as_ref() {
        let desc = query.error_description.clone().unwrap_or_default();
        let msg = format!("<h2>OAuth Error</h2><p>{}</p><p>{}</p>", err, desc);
        return (StatusCode::OK, Html(msg));
    }

    // If we got a code, pass it via the channel.
    if let Some(code) = query.code.clone() {
        if let Some(tx) = state.done_tx.lock().await.take() {
            let _ = tx.send(CallbackResult {
                code,
                state: query.state.clone(),
            });
        }
        // Show user a "success" page.
        let success = "<h2>Authentication Successful</h2><p>You can close this window now.</p>";
        return (StatusCode::OK, Html(success.to_string()));
    }

    // If we didn't get a code, show an error or instruct the user.
    let msg = "<h2>Missing 'code' query param</h2><p>Check logs or try again.</p>";
    (StatusCode::OK, Html(msg.to_string()))
}

/// Attempt to bind to the configured `port` to see if it's available.
/// Returns Ok(()) if we can bind, Err(...) if not.
pub async fn test_port_available(port: u16) -> Result<(), Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            // We only wanted to check binding, so close immediately.
            drop(listener);
            Ok(())
        }
        Err(e) => Err(Error::Auth(format!("Port {} not available: {}", port, e))),
    }
}

/// Very rough example of running `netstat` or similar to see what might be using the port.
/// In a real application, you might parse the output carefully or do platform-specific logic.
pub fn try_netstat_display(port: u16) {
    println!("Attempting to run 'netstat -an' to help identify the conflict:");
    let cmd = if cfg!(target_os = "windows") {
        "netstat -an"
    } else {
        "netstat -anp"
    };
    match Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains(&format!(":{}", port)) {
                    println!("{}", line);
                }
            }
        }
        Err(e) => {
            println!("Could not run netstat: {}", e);
        }
    }
}

/// Helper function to ensure we can use the stored callback port (from DB) or prompt the user
/// to change it if there's a conflict. Returns a final workable port.
pub async fn get_or_fix_callback_port<A>(
    port_repo: &A
) -> Result<u16, Error>
where
    A: AppConfigRepository,
{
    let existing = port_repo.get_callback_port().await?;
    let port = existing.unwrap_or(9876); // default fallback
    if let Err(e) = test_port_available(port).await {
        println!("Port conflict: {}", e);
        try_netstat_display(port);
        // Ask user if they want to enter a new port.
        println!("Do you want to change the callback port in the database? (Y/N)");
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_ok() {
            if line.trim().eq_ignore_ascii_case("y") {
                println!("Enter a new port number:");
                let mut new_line = String::new();
                if std::io::stdin().read_line(&mut new_line).is_ok() {
                    if let Ok(new_port) = new_line.trim().parse::<u16>() {
                        port_repo.set_callback_port(new_port).await?;
                        // Re-check the new port.
                        test_port_available(new_port).await.map_err(|e| {
                            Error::Auth(format!("Chosen port {} is still invalid: {}", new_port, e))
                        })?;
                        println!("Port {} is now stored in the DB and is available.", new_port);
                        return Ok(new_port);
                    }
                }
                return Err(Error::Auth("Invalid port entered.".into()));
            }
        }
        // If the user says 'no', we fail.
        return Err(Error::Auth("Cannot proceed with OAuth flow on a busy port. Aborting.".into()));
    }
    Ok(port)
}