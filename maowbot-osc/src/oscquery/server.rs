use crate::{Result, OscError};
use warp::Filter;
use std::net::{SocketAddr, Ipv4Addr};

pub struct OscQueryServer {
    is_running: bool,
    http_port: u16,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl OscQueryServer {
    pub fn new(port: u16) -> Self {
        Self {
            is_running: false,
            http_port: port,
            stop_tx: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }
        self.is_running = true;

        // Simple route returning some JSON:
        let route = warp::path::end().map(|| {
            warp::reply::json(&serde_json::json!({
                "hello": "from MaowBot",
                "addresses": ["/avatar/parameters/Example"]
            }))
        });

        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        self.stop_tx = Some(stop_tx);

        // Instead of ([0, 0, 0, 0], self.http_port),
        // make a proper SocketAddr (IPv4 or IPv6).
        let ip = Ipv4Addr::UNSPECIFIED;  // 0.0.0.0
        let addr = SocketAddr::new(ip.into(), self.http_port);

        // warp::serve(...).bind_with_graceful_shutdown(...) => (SocketAddr, impl Future)
        let (server_addr, server_future) = warp::serve(route)
            .bind_with_graceful_shutdown(addr, async move {
                let _ = stop_rx.await;
            });

        tracing::info!("Starting OSCQuery HTTP server on http://{}", server_addr);

        // Spawn the future in the background
        tokio::spawn(async move {
            server_future.await;
            tracing::info!("OSCQuery HTTP server shut down.");
        });

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }
        self.is_running = false;
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        Ok(())
    }
}
