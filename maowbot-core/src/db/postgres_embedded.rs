// maowbot-core/src/db/postgres_embedded.rs
//! Demonstrates how to spin up an embedded Postgres instance for tests using pg-embed v0.8+.
//!
//! If you don't need embedded Postgres, you can remove or ignore this file.

use std::path::PathBuf;
use anyhow::Result;
use pg_embed::{
    pg_enums::{PgAuthMethod, OperationSystem, Architecture},
    postgres::{PgEmbed, PgSettings},
};
use pg_embed::pg_fetch;
use pg_embed::pg_fetch::{PgFetchSettings, PostgresVersion, PG_V15};
use serde_json::to_string;
use serenity::all::NsfwLevel::Default;
use tempfile::tempdir;
use crate::Error;

/// A helper to manage embedded Postgres for tests.
pub struct EmbeddedPg {
    /// The pg-embed manager.
    pg: PgEmbed,
}

impl EmbeddedPg {
    /// Starts an embedded Postgres instance. Adjust fields as needed.
    pub async fn start() -> Result<Self> {
        // Create a temporary directory for the database files.

        let data_dir = PathBuf::from("data/");

        let settings = PgSettings {
            database_dir: data_dir.clone(),
            port: 54321,
            user: "postgres".into(),
            password: "postgres".into(),
            auth_method: PgAuthMethod::Plain,
            persistent: true,
            migration_dir: Some(PathBuf::from("../migrations")),
            timeout: Some(std::time::Duration::from_secs(30)),
        };

        // Provide the necessary fetch settings:
        let fetch_settings = PgFetchSettings {
            host: "https://repo1.maven.org".to_string(),
            operating_system: OperationSystem::Windows,
            architecture: Architecture::Amd64,
            version: PG_V15,
        };

        // Create the PgEmbed instance (async in v0.8+).
        let mut pg = PgEmbed::new(settings, fetch_settings).await?;
        // Start the database.
        pg.start_db().await?;

        Ok(Self {
            pg,
        })
    }

    /// Stops the embedded Postgres instance.
    pub async fn stop(&mut self) -> Result<(), Error> {
        self.pg.stop_db().await?;
        Ok(())
    }

    /// Returns the connection string for the embedded database.
    pub fn connection_string(&self) -> String {
        self.pg.db_uri.clone()
    }
}
