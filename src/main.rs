use maowbot::{Database};
use std::time::Duration;
use tokio::task;
use tracing::{info, error};
use tracing_subscriber::FmtSubscriber;

use maowbot::tasks::credential_refresh;
use maowbot::auth::{AuthManager, StubAuthHandler};
use maowbot::crypto::Encryptor;
use maowbot::repositories::sqlite::SqliteCredentialsRepository;
use std::sync::Arc;
use tokio::sync::Mutex;

fn init_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber for tracing");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    info!("Starting maowbot...");

    let db = Database::new("data/bot.db").await?;
    db.migrate().await?;
    info!("Database initialized and migrated successfully!");

    // Encryption key
    let key = [0u8; 32];
    let encryptor = Encryptor::new(&key)?;

    // Create credentials repository
    let creds_repo = SqliteCredentialsRepository::new(db.pool().clone(), encryptor);

    // Create AuthManager
    let auth_manager = AuthManager::new(
        Box::new(creds_repo),
        Box::new(StubAuthHandler::default())
    );

    let auth_manager = Arc::new(Mutex::new(auth_manager));

    // Example background task for token refresh
    let creds_repo_clone =
        SqliteCredentialsRepository::new(db.pool().clone(), Encryptor::new(&key)?);
    let auth_manager_clone = auth_manager.clone();

    task::spawn(async move {
        let check_interval = Duration::from_secs(300); // 5 minutes
        loop {
            let within_minutes = 10;
            let mut am = auth_manager_clone.lock().await;
            match credential_refresh::refresh_expiring_tokens(
                &creds_repo_clone,
                &mut am,
                within_minutes
            )
                .await
            {
                Ok(_) => info!("Finished refresh_expiring_tokens cycle."),
                Err(e) => error!("Error refreshing tokens: {:?}", e),
            }
            tokio::time::sleep(check_interval).await;
        }
    });

    info!("Main logic initialization complete. Running...");
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
