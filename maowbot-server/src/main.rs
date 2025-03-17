//! maowbot-server/src/main.rs
//!
//! Entry point: parse CLI args, initialize tracing, then call run_server or run_client.

use clap::Parser;
use std::error::Error as StdError;
use tracing::{info, error};
use tracing_subscriber::{EnvFilter};

#[derive(Parser, Debug, Clone)]
#[command(name = "maowbot")]
#[command(author, version, about = "MaowBot - multi‑platform streaming bot with plugin system")]
pub struct Args {
    /// Mode: "server" or "client"
    #[arg(long, default_value = "server")]
    pub mode: String,

    /// Address to which the server will bind
    #[arg(long, default_value = "0.0.0.0:9999")]
    pub server_addr: String,

    /// Postgres connection URL.
    #[arg(long, default_value = "postgres://maow@localhost:5432/maowbot")]
    pub db_path: String,

    /// Passphrase for plugin connections
    #[arg(long)]
    pub plugin_passphrase: Option<String>,

    /// Path to an in‑process plugin .so/.dll (optional)
    #[arg(long)]
    pub in_process_plugin: Option<String>,

    /// If you want to run the TUI interface in the console
    #[arg(long, short = 't', default_value = "false")]
    pub tui: bool,

    /// If you want to run in headless mode
    #[arg(long, default_value = "false")]
    pub headless: bool,

    #[arg(long, default_value = "false")]
    pub auth: bool,

    /// Logging level: "info", "warn", "debug", "error", or "trace"
    #[arg(long = "log-level", short = 'L', default_value = "info", value_parser = ["info", "warn", "debug", "error", "trace"])]
    pub log_level: String,
}

fn init_tracing(level: &str) {
    let default_filter = format!("maowbot={0},twitch_irc={0}", level);
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global subscriber");
    tracing_log::LogTracer::init().ok();
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn StdError>> {
    let args = Args::parse();
    init_tracing(&args.log_level);

    info!(
        "MaowBot starting. mode={}, headless={}, tui={}, auth={}",
        args.mode, args.headless, args.tui, args.auth
    );

    match args.mode.as_str() {
        "server" => {
            if let Err(e) = crate::server::run_server(args).await {
                error!("Server error: {:?}", e);
            }
        }
        "client" => {
            if let Err(e) = crate::client::run_client(args).await {
                error!("Client error: {:?}", e);
            }
        }
        other => {
            error!("Invalid mode '{}'. Use --mode=server or --mode=client.", other);
        }
    }

    info!("Main finished. Goodbye!");
    Ok(())
}

// Bring in the rest of our modules
mod context;
mod server;
mod client;
pub mod portable_postgres;