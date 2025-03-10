//! maowbot-server/src/server.rs
//!
//! The main server logic: building the ServerContext and running the gRPC plugin service.

use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time;
use tracing::{info, error};
use tonic::transport::{Server, Identity, Certificate, ServerTlsConfig};
use std::fs;
use std::path::Path;
use std::io::Write;
use rcgen::{generate_simple_self_signed};
use maowbot_core::Error;
use maowbot_core::eventbus::{BotEvent};
use maowbot_core::eventbus::db_logger::{spawn_db_logger_task};
use maowbot_core::eventbus::db_logger_handle::DbLoggerControl;
use maowbot_core::plugins::service_grpc::PluginServiceGrpc;
use maowbot_proto::plugs::plugin_service_server::PluginServiceServer;

use crate::Args;
use crate::context::ServerContext;
use crate::portable_postgres::*;
use maowbot_core::tasks::biweekly_maintenance::{
    spawn_biweekly_maintenance_task
};
use maowbot_core::tasks::credential_refresh::refresh_all_refreshable_credentials;
use maowbot_core::tasks::autostart::run_autostart;
use maowbot_core::tasks::redeem_sync;
use maowbot_tui::TuiModule;

pub async fn run_server(args: Args) -> Result<(), Error> {
    // Build the global context
    let ctx = ServerContext::new(&args).await?;

    // Start your OSC server on a free port:
    match ctx.osc_manager.start_server().await {
        Ok(port) => {
            tracing::info!("OSC server is running on port {}", port);
        }
        Err(e) => {
            tracing::error!("Failed to start OSC server: {:?}", e);
        }
    }

    // 1) Spawn DB logger
    let (db_logger_handle, db_logger_control) = start_db_logger(&ctx);
    // 2) Spawn maintenance
    let _maintenance_task = spawn_biweekly_maintenance_task(
        ctx.db.clone(),
        maowbot_core::repositories::postgres::user_analysis::PostgresUserAnalysisRepository::new(ctx.db.pool().clone()),
        ctx.event_bus.clone()
    );

    redeem_sync::sync_channel_redeems(
        &ctx.redeem_service,
        &ctx.platform_manager,
        &ctx.message_service.user_service,
        &*ctx.bot_config_repo.clone(),
        false
    ).await?;

    // 3) Refresh credentials
    {
        let mut auth_lock = ctx.auth_manager.lock().await;
        if let Err(e) = refresh_all_refreshable_credentials(ctx.creds_repo.as_ref(), &mut *auth_lock).await {
            error!("Failed to refresh credentials on startup => {:?}", e);
        }
    }

    // 4) Autostart any configured accounts
    let bot_api = Arc::new(ctx.plugin_manager.clone());
    if let Err(e) = run_autostart(ctx.bot_config_repo.as_ref(), bot_api.clone()).await {
        error!("Autostart error => {:?}", e);
    }

    // 5) If TUI was requested
    if args.tui {
        let tui_module = Arc::new(TuiModule::new(bot_api.clone(), ctx.event_bus.clone()).await);
        tui_module.spawn_tui_thread().await;
    }

    // Let active plugins see the BotApi
    {
        let lock = ctx.plugin_manager.plugins.lock().await;
        for p in lock.iter() {
            p.set_bot_api(bot_api.clone());
        }
    }

    let eventsub_svc_clone = ctx.eventsub_service.clone();
    tokio::spawn(async move {
        eventsub_svc_clone.start().await;
    });

    // 6) Start the gRPC server
    let identity = load_or_generate_certs()?;
    let tls_config = ServerTlsConfig::new().identity(identity);
    let addr: SocketAddr = args.server_addr.parse()?;
    info!("Starting Tonic gRPC server on {}", addr);

    let service_impl = PluginServiceGrpc {
        manager: Arc::new(ctx.plugin_manager.clone()),
    };
    let server_future = Server::builder()
        .tls_config(tls_config)?
        .add_service(PluginServiceServer::new(service_impl))
        .serve(addr);

    let event_bus = ctx.event_bus.clone();
    let srv_handle = tokio::spawn(async move {
        if let Err(e) = server_future.await {
            error!("gRPC server error: {:?}", e);
        }
    });

    // Ctrl-C => signal
    let eb_for_ctrlc = event_bus.clone();
    let _ctrlc_handle = tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl‑C: {:?}", e);
        }
        info!("Ctrl‑C detected; shutting down event bus...");
        eb_for_ctrlc.shutdown();
    });

    // 7) Main loop => send Tick events until we see shutdown
    let mut shutdown_rx = event_bus.shutdown_rx.clone();
    loop {
        tokio::select! {
            _ = time::sleep(Duration::from_secs(10)) => {
                event_bus.publish(BotEvent::Tick).await;
            }
            Ok(_) = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Shutdown signaled; exiting server loop.");
                    break;
                }
            }
        }
    }

    // Cleanup
    info!("Stopping gRPC server...");
    srv_handle.abort();
    info!("Stopping Postgres...");
    ctx.stop_postgres();
    info!("Server shutdown complete.");

    // Ensure DB logger is done
    db_logger_handle.abort();

    Ok(())
}

/// Spawns the DB-logger task, returns (JoinHandle, DbLoggerControl).
fn start_db_logger(ctx: &ServerContext) -> (tokio::task::JoinHandle<()>, DbLoggerControl) {
    let (jh, control) = spawn_db_logger_task(
        &ctx.event_bus,
        maowbot_core::repositories::postgres::analytics::PostgresAnalyticsRepository::new(ctx.db.pool().clone()),
        100,
        5,
    );
    (jh, control)
}

/// Load or generate self-signed TLS cert for gRPC.
fn load_or_generate_certs() -> Result<Identity, Error> {
    let cert_folder = "certs";
    let cert_path = format!("{}/server.crt", cert_folder);
    let key_path  = format!("{}/server.key", cert_folder);

    if Path::new(&cert_path).exists() && Path::new(&key_path).exists() {
        let cert_pem = fs::read(&cert_path)?;
        let key_pem  = fs::read(&key_path)?;
        return Ok(Identity::from_pem(cert_pem, key_pem));
    }

    let alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string(), "0.0.0.0".to_string()];
    let certified = generate_simple_self_signed(alt_names)?;
    let cert_pem = certified.cert.pem();
    let key_pem = certified.key_pair.serialize_pem();

    fs::create_dir_all(cert_folder)?;
    fs::File::create(&cert_path)?.write_all(cert_pem.as_bytes())?;
    fs::File::create(&key_path)?.write_all(key_pem.as_bytes())?;

    Ok(Identity::from_pem(cert_pem, key_pem))
}
