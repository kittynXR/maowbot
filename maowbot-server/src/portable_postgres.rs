// maowbot-server/src/portable_postgres.rs
use std::path::Path;
use std::process::Command;
use std::io::Result as IoResult;
use std::time::Duration;
use std::thread;
use tracing::{info, error};

/// Check if the data folder is initialized. If not, run initdb
/// with superuser "maow" and trust authentication so that role "maow" will exist.
///
/// We now also add `--encoding=UTF8` and `--locale=en_US.UTF-8` so the cluster is fully UTF-8.
pub fn ensure_db_initialized(pg_bin_dir: &str, data_dir: &str) -> IoResult<()> {
    let version_file = format!("{}/PG_VERSION", data_dir);
    if !Path::new(&version_file).exists() {
        info!("No PG_VERSION found in '{}'. Running initdb with UTF-8...", data_dir);

        let initdb_path = format!("{}/initdb", pg_bin_dir);
        let status = Command::new(&initdb_path)
            .args(&[
                "-D", data_dir,
                "-U", "maow",
                "-A", "trust",
                "--encoding=UTF8",         // <-- ensures cluster is created in UTF-8
                "--locale=en_US.UTF-8",    // <-- typical locale that supports UTF-8
            ])
            .status()?;

        if !status.success() {
            error!("initdb failed with status: {:?}", status);
        } else {
            info!("initdb completed successfully (UTF-8).");
        }
    }
    Ok(())
}

/// Start Postgres on the given port, logging to server.log in data_dir.
pub fn start_postgres(pg_bin_dir: &str, data_dir: &str, port: u16) -> IoResult<()> {
    let pg_ctl_path = format!("{}/pg_ctl", pg_bin_dir);
    let log_file = format!("{}/server.log", data_dir);

    let status = Command::new(&pg_ctl_path)
        .args(&[
            "start",
            "-D", data_dir,
            "-o", &format!("-p {}", port),
            "-l", &log_file,
        ])
        .status()?;

    if !status.success() {
        error!("pg_ctl start failed with status: {:?}", status);
    } else {
        info!("Postgres started on port {}.", port);
        thread::sleep(Duration::from_secs(1));
    }

    Ok(())
}

/// Creates (or ensures) a database named `db_name`. We add `-E UTF8` so the DB is UTF-8 encoded.
/// We also add `--template=template0` so the new database is created from template0 rather than template1.
/// This avoids the encoding mismatch error.
pub fn create_database(pg_bin_dir: &str, port: u16, db_name: &str) -> std::io::Result<()> {
    let createdb_path = format!("{}/createdb", pg_bin_dir);
    info!("Ensuring database '{}' exists with UTF-8 encoding...", db_name);

    // Add --template=template0 and -E UTF8 to force the new database to be UTF-8.
    let status = Command::new(&createdb_path)
        .args([
            "-U", "maow",
            "-p", &port.to_string(),
            "--template=template0",   // <-- Use template0 for UTF8 support
            "-E", "UTF8",
            db_name,
        ])
        .status()?;

    if status.success() {
        info!("Database '{}' created (UTF-8).", db_name);
    } else {
        info!(
            "Database '{}' may already exist or could not be created (exit status: {:?}). Continuing...",
            db_name, status
        );
    }
    Ok(())
}

/// Stop Postgres gracefully.
pub fn stop_postgres(pg_bin_dir: &str, data_dir: &str) -> IoResult<()> {
    let pg_ctl_path = format!("{}/pg_ctl", pg_bin_dir);
    info!("Stopping Postgres...");
    let status = Command::new(&pg_ctl_path)
        .args(&["stop", "-D", data_dir, "-m", "fast"])
        .status()?;

    if !status.success() {
        error!("pg_ctl stop returned status: {:?}", status);
    } else {
        info!("Postgres stopped.");
    }
    Ok(())
}