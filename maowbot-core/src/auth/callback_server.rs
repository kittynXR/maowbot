use std::{net::SocketAddr, sync::Arc, process::Command};
use tokio::sync::{oneshot, Mutex};
use axum::{
    Router,
    routing::get,
    extract::{Query, State},
    response::Html,
    http::StatusCode,
};
use axum_server::{Server, Handle};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use serde::Deserialize;
use tracing::{info, error};

use crate::Error;
use crate::repositories::postgres::bot_config::BotConfigRepository;

/// Structure to hold the final result from the OAuth callback.
#[derive(Debug, Clone)]
pub struct CallbackResult {
    pub code: String,
    pub state: Option<String>,
}

/// Query string we expect from e.g. Twitch: ?code=xxx&state=...
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

pub async fn start_callback_server(
    port: u16
) -> Result<(oneshot::Receiver<CallbackResult>, oneshot::Sender<()>), Error> {
    let (done_tx, done_rx) = oneshot::channel::<CallbackResult>();
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));

    let state = CallbackServerState { done_tx };

    let app = Router::new()
        .route("/callback", get(handle_callback))
        .with_state(state)
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let (shutdown_send, shutdown_recv) = oneshot::channel::<()>();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("OAuth callback server listening on http://{}", addr);

    let handle = Handle::new();
    let handle_clone = handle.clone();

    tokio::spawn(async move {
        let _ = shutdown_recv.await;
        handle_clone.graceful_shutdown(None);
    });

    let server = Server::bind(addr)
        .handle(handle)
        .serve(app.into_make_service());

    tokio::spawn(async move {
        if let Err(e) = server.await {
            error!("Callback server error: {}", e);
        }
        info!("Callback server shut down.");
    });

    Ok((done_rx, shutdown_send))
}

async fn handle_callback(
    State(state): State<CallbackServerState>,
    Query(query): Query<AuthQuery>,
) -> (StatusCode, Html<String>) {
    if let Some(err) = query.error.as_ref() {
        let desc = query.error_description.clone().unwrap_or_default();
        let msg = format!("<h2>OAuth Error</h2><p>{}</p><p>{}</p>", err, desc);
        return (StatusCode::OK, Html(msg));
    }

    if let Some(code) = query.code.clone() {
        if let Some(tx) = state.done_tx.lock().await.take() {
            let _ = tx.send(CallbackResult {
                code,
                state: query.state.clone(),
            });
        }

        // A snippet that tries to auto-close the browser tab:
        let success = r#"
<h2>Authentication Successful</h2>
<p>We've got your code. You can close this window now.</p>
<script>
  // Attempt to close the tab automatically
  window.onload = function() {
      window.open('about:blank', '_self');
      window.close();
  };
</script>
"#;
        return (StatusCode::OK, Html(success.to_string()));
    }

    let msg = "<h2>Missing 'code' query param</h2><p>Check logs or try again.</p>";
    (StatusCode::OK, Html(msg.to_string()))
}

pub async fn test_port_available(port: u16) -> Result<(), Error> {
    use tokio::net::TcpListener;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(e) => Err(Error::Auth(format!("Port {} not available: {}", port, e))),
    }
}

pub fn try_netstat_display(port: u16) {
    println!("Attempting to run 'netstat -an' to help identify the conflict:");
    let cmd = if cfg!(target_os = "windows") {
        "netstat -an"
    } else {
        "netstat -anp"
    };
    match Command::new("sh").arg("-c").arg(cmd).output() {
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

/// If you'd like to read or fix the callback port in the database. Not used by Twitch anymore.
pub async fn get_or_fix_callback_port(
    port_repo: &(dyn BotConfigRepository + Send + Sync)
) -> Result<u16, Error> {
    let existing = port_repo.get_callback_port().await?;
    let port = existing.unwrap_or(9876); // default fallback
    if let Err(e) = test_port_available(port).await {
        println!("Port conflict: {}", e);
        try_netstat_display(port);
        println!("Do you want to change the callback port in the database? (Y/N)");
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_ok() {
            if line.trim().eq_ignore_ascii_case("y") {
                println!("Enter a new port number:");
                let mut new_line = String::new();
                if std::io::stdin().read_line(&mut new_line).is_ok() {
                    if let Ok(new_port) = new_line.trim().parse::<u16>() {
                        port_repo.set_callback_port(new_port).await?;
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
        return Err(Error::Auth("Cannot proceed with OAuth flow on a busy port. Aborting.".into()));
    }
    Ok(port)
}