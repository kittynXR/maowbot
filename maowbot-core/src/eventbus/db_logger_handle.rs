// File: maowbot-core/src/eventbus/db_logger_handle.rs
//
// A small control handle for forcing flushes in the db_logger task.

use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::trace;
use crate::Error;

/// Commands we can send to the db_logger's main loop.
#[derive(Debug)]
pub enum DbLoggerCommand {
    /// Force a synchronous flush of the current buffer. If successful,
    /// all queued messages are inserted. The oneshot is signalled on completion.
    FlushNow(oneshot::Sender<Result<(), Error>>),

    // Optionally, you can add other commands later if needed...
}

/// A handle that we can clone and keep in the user API or plugin manager,
/// allowing them to call `.flush_now()` at any time.
#[derive(Clone)]
pub struct DbLoggerControl {
    cmd_tx: mpsc::Sender<DbLoggerCommand>,
}

impl DbLoggerControl {
    pub fn new(cmd_tx: mpsc::Sender<DbLoggerCommand>) -> Self {
        Self { cmd_tx }
    }

    /// Request a forced flush of all queued chat messages in the db_logger.
    /// This returns an error if the db_logger task is gone or if the flush
    /// fails internally.
    pub async fn flush_now(&self) -> Result<(), Error> {
        trace!("DbLoggerControl: flush_now() called.");
        let (reply_tx, reply_rx) = oneshot::channel();

        // Send the flush command
        self.cmd_tx
            .send(DbLoggerCommand::FlushNow(reply_tx))
            .await
            .map_err(|_| Error::EventBus("db_logger task is not running?".into()))?;

        // Wait for it to complete
        match reply_rx.await {
            Ok(res) => res,
            Err(_) => Err(Error::EventBus(
                "db_logger forced flush was dropped.".into()
            )),
        }
    }
}