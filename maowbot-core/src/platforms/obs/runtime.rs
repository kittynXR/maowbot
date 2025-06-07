use crate::{Error, crypto::Encryptor};
use crate::eventbus::{EventBus, BotEvent};
use crate::repositories::postgres::obs::PostgresObsRepository;
use maowbot_common::traits::repository_traits::ObsRepository;
use async_trait::async_trait;
use maowbot_common::models::platform::Platform;
use maowbot_common::traits::platform_traits::{PlatformIntegration, PlatformAuth, ConnectionStatus};
use maowbot_obs::{ObsClient, ObsInstance};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub struct ObsRuntime {
    instance_number: u32,
    user_id: Uuid,
    client: Arc<ObsClient>,
    status: Arc<RwLock<ConnectionStatus>>,
    event_bus: Arc<EventBus>,
    repository: PostgresObsRepository,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Arc<RwLock<Option<mpsc::Receiver<()>>>>,
}

impl ObsRuntime {
    pub async fn new(
        instance_number: u32,
        user_id: Uuid,
        pool: Pool<Postgres>,
        event_bus: Arc<EventBus>,
        encryptor: Encryptor,
    ) -> Result<Self, Error> {
        let repository = PostgresObsRepository::new(pool, encryptor);
        
        // Get instance config from database
        let instance = repository.get_instance(instance_number)
            .await?
            .ok_or_else(|| Error::NotFound(format!("OBS instance {} not found", instance_number)))?;
        
        let client = Arc::new(ObsClient::new(instance));
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        
        Ok(Self {
            instance_number,
            user_id,
            client,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            event_bus,
            repository,
            shutdown_tx,
            shutdown_rx: Arc::new(RwLock::new(Some(shutdown_rx))),
        })
    }
    
    async fn connection_loop(&self) {
        let mut shutdown_rx = self.shutdown_rx.write().await.take().unwrap();
        let mut reconnect_delay = Duration::from_secs(1);
        
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("OBS instance {} shutting down", self.instance_number);
                    break;
                }
                _ = self.connect_with_retry(&mut reconnect_delay) => {
                    // Connection lost, will retry
                }
            }
        }
        
        // Clean disconnect
        let _ = self.client.disconnect().await;
        let _ = self.repository.set_connection_status(self.instance_number, false).await;
        *self.status.write().await = ConnectionStatus::Disconnected;
    }
    
    async fn connect_with_retry(&self, reconnect_delay: &mut Duration) {
        match self.client.connect().await {
            Ok(_) => {
                info!("Connected to OBS instance {}", self.instance_number);
                *self.status.write().await = ConnectionStatus::Connected;
                let _ = self.repository.set_connection_status(self.instance_number, true).await;
                
                // Reset reconnect delay on successful connection
                *reconnect_delay = Duration::from_secs(1);
                
                // Emit connection event
                self.event_bus.publish(BotEvent::SystemMessage(
                    format!("OBS instance {} connected", self.instance_number)
                )).await;
                
                // Wait for disconnect or shutdown
                loop {
                    if !self.client.is_connected().await {
                        warn!("OBS instance {} disconnected", self.instance_number);
                        break;
                    }
                    sleep(Duration::from_secs(5)).await;
                }
                
                // Emit disconnection event
                self.event_bus.publish(BotEvent::SystemMessage(
                    format!("OBS instance {} disconnected", self.instance_number)
                )).await;
            }
            Err(e) => {
                error!("Failed to connect to OBS instance {}: {}", self.instance_number, e);
                *self.status.write().await = ConnectionStatus::Error(e.to_string());
                
                // Exponential backoff with max delay of 60 seconds
                sleep(*reconnect_delay).await;
                *reconnect_delay = (*reconnect_delay * 2).min(Duration::from_secs(60));
            }
        }
    }
    
    pub fn get_client(&self) -> Arc<ObsClient> {
        self.client.clone()
    }
}

#[async_trait]
impl PlatformAuth for ObsRuntime {
    async fn authenticate(&mut self) -> Result<(), Error> {
        // OBS authentication is handled during connection
        Ok(())
    }
    
    async fn refresh_auth(&mut self) -> Result<(), Error> {
        // OBS doesn't have refresh tokens
        Ok(())
    }
    
    async fn revoke_auth(&mut self) -> Result<(), Error> {
        // No token revocation for OBS
        Ok(())
    }
    
    async fn is_authenticated(&self) -> Result<bool, Error> {
        // Check if we have valid instance configuration
        Ok(true)
    }
}

#[async_trait]
impl PlatformIntegration for ObsRuntime {
    async fn connect(&mut self) -> Result<(), Error> {
        let runtime = self.clone();
        tokio::spawn(async move {
            runtime.connection_loop().await;
        });
        
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<(), Error> {
        info!("Disconnecting OBS instance {}", self.instance_number);
        let _ = self.shutdown_tx.send(()).await;
        Ok(())
    }
    
    async fn send_message(&self, channel: &str, message: &str) -> Result<(), Error> {
        // OBS doesn't send chat messages
        Err(Error::Platform("OBS does not support sending messages".into()))
    }
    
    async fn get_connection_status(&self) -> Result<ConnectionStatus, Error> {
        Ok(self.status.read().await.clone())
    }
}

impl Clone for ObsRuntime {
    fn clone(&self) -> Self {
        Self {
            instance_number: self.instance_number,
            user_id: self.user_id,
            client: self.client.clone(),
            status: self.status.clone(),
            event_bus: self.event_bus.clone(),
            repository: self.repository.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
        }
    }
}