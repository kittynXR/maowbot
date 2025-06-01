// maowbot-server/src/portable_postgres.rs
use std::{
    fs::File,
    io::Read,
    path::Path,
    process::Command,
};
use std::io::Result as IoResult;
use std::process::Stdio;
use std::time::Duration;
use std::thread;
use tokio::time::Instant;
use tracing::{info, error, warn};

// ---------------------------------------------------------------------
// Windows (actual embedded Postgres logic)
// ---------------------------------------------------------------------
#[cfg(windows)]
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
                "--encoding=UTF8",
                "--locale=en_US.UTF-8",
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

#[cfg(windows)]
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

#[cfg(windows)]
/// Creates (or ensures) a database named `db_name`. We add `-E UTF8` so the DB is UTF-8.
/// Also add `--template=template0` so the new database is created from template0.
pub fn create_database(pg_bin_dir: &str, port: u16, db_name: &str) -> IoResult<()> {
    let createdb_path = format!("{}/createdb", pg_bin_dir);
    info!("Ensuring database '{}' exists with UTF-8 encoding...", db_name);

    let status = Command::new(&createdb_path)
        .args([
            "-U", "maow",
            "-p", &port.to_string(),
            "--template=template0",
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

#[cfg(windows)]
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

// ---------------------------------------------------------------------
// Non‐Windows (stubbed versions – skip embedded Postgres entirely)
// ---------------------------------------------------------------------
#[cfg(not(windows))]
pub fn ensure_db_initialized(_pg_bin_dir: &str, data_dir: &str) -> IoResult<()> {
    info!(
        "Skipping embedded initdb on non-Windows. Please ensure Postgres is installed and that '{}' is valid if used.",
        data_dir
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn start_postgres(_pg_bin_dir: &str, data_dir: &str, port: u16) -> IoResult<()> {
    info!(
        "Skipping embedded postgres start on non-Windows. Using system Postgres on port {} (data_dir: {}).",
        port, data_dir
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn create_database(_pg_bin_dir: &str, _port: u16, db_name: &str) -> IoResult<()> {
    info!(
        "Skipping embedded createdb on non-Windows. Make sure '{}' exists in system Postgres if needed.",
        db_name
    );
    Ok(())
}

#[cfg(not(windows))]
pub fn stop_postgres(_pg_bin_dir: &str, data_dir: &str) -> IoResult<()> {
    info!(
        "Skipping embedded pg_ctl stop on non-Windows. If a local DB is running, you must manage it yourself (data_dir: {}).",
        data_dir
    );
    Ok(())
}

// ---------------------------------------------------------------------
// The rest (kill leftover Postgres, etc.) can remain OS‐specific as is
// or you can also stub them out the same way if you like.
// For illustration, we’ll keep them as is, since they’re mostly no-ops
// on Windows vs. Unix anyway.
// ---------------------------------------------------------------------

#[cfg(windows)]
fn kill_process(pid: u32) -> IoResult<()> {
    warn!("Force-killing leftover Postgres (PID={}) on Windows...", pid);
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .status()?;
    Ok(())
}

#[cfg(unix)]
fn kill_process(pid: u32) -> IoResult<()> {
    warn!("Force-killing leftover Postgres (PID={}) on Unix...", pid);
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()?;
    Ok(())
}

/// Attempt a fast shutdown via `pg_ctl stop -m fast -D data_dir`.
/// Returns `Ok(true)` if it stopped within 3s, or `Ok(false)` if we had to kill it.
fn attempt_pg_ctl_fast_stop(pg_bin_dir: &str, data_dir: &str) -> IoResult<bool> {
    let pg_ctl_path = format!("{}/pg_ctl", pg_bin_dir);

    let mut child = Command::new(&pg_ctl_path)
        .args(&["stop", "-D", data_dir, "-m", "fast"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    info!("Attempting fast shutdown of leftover Postgres via pg_ctl stop...");

    let start = Instant::now();
    let timeout = Duration::from_secs(3);

    loop {
        match child.try_wait()? {
            Some(status) => {
                if status.success() {
                    info!("pg_ctl stop -m fast completed successfully.");
                } else {
                    warn!("pg_ctl stop -m fast returned a non-success exit code.");
                }
                return Ok(true);
            }
            None => {
                if start.elapsed() >= timeout {
                    warn!("pg_ctl stop timed out; killing pg_ctl...");
                    child.kill()?;
                    return Ok(false);
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// If a stale `postmaster.pid` is found in `data_dir`, try a fast `pg_ctl stop`.
/// If it doesn’t work, forcibly kill the leftover PID.
pub fn kill_leftover_postgres_if_any(pg_bin_dir: &str, data_dir: &str) -> IoResult<()> {
    let pid_file = format!("{}/postmaster.pid", data_dir);
    let pid_path = Path::new(&pid_file);

    if !pid_path.exists() {
        return Ok(());
    }

    info!("Found leftover postmaster.pid in {}; reading...", pid_file);

    let mut contents = String::new();
    File::open(pid_path)?.read_to_string(&mut contents)?;
    let lines: Vec<&str> = contents.lines().collect();
    if lines.is_empty() {
        warn!("postmaster.pid is empty/corrupt; removing it.");
        std::fs::remove_file(pid_path)?;
        return Ok(());
    }

    let leftover_pid: u32 = match lines[0].trim().parse() {
        Ok(pid) => pid,
        Err(_) => {
            warn!("Could not parse leftover PID from first line; removing file.");
            std::fs::remove_file(pid_path)?;
            return Ok(());
        }
    };

    if lines.len() >= 2 {
        let leftover_dir = lines[1].trim();
        let leftover_abs = std::fs::canonicalize(leftover_dir).unwrap_or_default();
        let our_abs = std::fs::canonicalize(data_dir).unwrap_or_default();
        if leftover_abs != our_abs {
            warn!("postmaster.pid belongs to a different data-dir, skipping forced kill.");
            return Ok(());
        }
    } else {
        warn!("postmaster.pid has no data-dir line; continuing anyway.");
    }

    info!("Leftover Postgres PID={} (data-dir={}); attempting graceful shutdown...", leftover_pid, data_dir);
    match attempt_pg_ctl_fast_stop(pg_bin_dir, data_dir)? {
        true => {
            std::fs::remove_file(pid_path)?;
            info!("Removed stale PID file; fast shutdown completed.");
        }
        false => {
            kill_process(leftover_pid)?;
            std::fs::remove_file(pid_path)?;
            info!("Removed stale PID file; force-killed leftover Postgres PID={}.", leftover_pid);
        }
    }

    Ok(())
}
