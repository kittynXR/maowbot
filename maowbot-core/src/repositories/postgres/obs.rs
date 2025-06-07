// File: maowbot-core/src/repositories/postgres/obs.rs

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use tracing::{debug, info};

use maowbot_common::error::Error;
use maowbot_common::traits::repository_traits::ObsRepository;
use maowbot_obs::ObsInstance;
use crate::crypto::Encryptor;

#[derive(Clone)]
pub struct PostgresObsRepository {
    pool: Pool<Postgres>,
    encryptor: Encryptor,
}

impl PostgresObsRepository {
    pub fn new(pool: Pool<Postgres>, encryptor: Encryptor) -> Self {
        Self { pool, encryptor }
    }
}

#[async_trait]
impl ObsRepository for PostgresObsRepository {
    async fn get_instance(&self, instance_number: u32) -> Result<Option<ObsInstance>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT instance_number, host, port, use_ssl, password_encrypted, use_password
            FROM obs_instances
            WHERE instance_number = $1
            "#
        )
        .bind(instance_number as i32)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            let instance_number: i32 = row.get("instance_number");
            let host: String = row.get("host");
            let port: i32 = row.get("port");
            let use_ssl: bool = row.get("use_ssl");
            let password_encrypted: Option<String> = row.get("password_encrypted");
            let use_password: bool = row.get("use_password");
            
            // Decrypt password if present
            let password = match password_encrypted {
                Some(encrypted) => Some(self.encryptor.decrypt(&encrypted)?),
                None => None,
            };
            
            let instance = ObsInstance {
                instance_number: instance_number as u32,
                host,
                port: port as u16,
                use_ssl,
                password,
                use_password,
            };
            
            Ok(Some(instance))
        } else {
            Ok(None)
        }
    }
    
    async fn update_instance(&self, instance: &ObsInstance) -> Result<(), Error> {
        // Encrypt password before storing
        let encrypted_password = match &instance.password {
            Some(password) => Some(self.encryptor.encrypt(password)?),
            None => None,
        };
        
        sqlx::query(
            r#"
            UPDATE obs_instances
            SET host = $2, 
                port = $3, 
                use_ssl = $4, 
                password_encrypted = $5, 
                use_password = $6,
                updated_at = NOW()
            WHERE instance_number = $1
            "#
        )
        .bind(instance.instance_number as i32)
        .bind(&instance.host)
        .bind(instance.port as i32)
        .bind(instance.use_ssl)
        .bind(encrypted_password)
        .bind(instance.use_password)
        .execute(&self.pool)
        .await?;
        
        info!("Updated OBS instance {} configuration", instance.instance_number);
        Ok(())
    }
    
    async fn set_connection_status(&self, instance_number: u32, connected: bool) -> Result<(), Error> {
        if connected {
            sqlx::query(
                r#"
                UPDATE obs_instances
                SET is_connected = true, 
                    last_connected_at = NOW()
                WHERE instance_number = $1
                "#
            )
            .bind(instance_number as i32)
            .execute(&self.pool)
            .await?;
            
            debug!("OBS instance {} marked as connected", instance_number);
        } else {
            sqlx::query(
                r#"
                UPDATE obs_instances
                SET is_connected = false
                WHERE instance_number = $1
                "#
            )
            .bind(instance_number as i32)
            .execute(&self.pool)
            .await?;
            
            debug!("OBS instance {} marked as disconnected", instance_number);
        }
        
        Ok(())
    }
    
    async fn list_instances(&self) -> Result<Vec<ObsInstance>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT instance_number, host, port, use_ssl, password_encrypted, use_password
            FROM obs_instances
            ORDER BY instance_number
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut instances = Vec::new();
        
        for row in rows {
            let instance_number: i32 = row.get("instance_number");
            let host: String = row.get("host");
            let port: i32 = row.get("port");
            let use_ssl: bool = row.get("use_ssl");
            let password_encrypted: Option<String> = row.get("password_encrypted");
            let use_password: bool = row.get("use_password");
            
            // Decrypt password if present
            let password = match password_encrypted {
                Some(encrypted) => Some(self.encryptor.decrypt(&encrypted)?),
                None => None,
            };
            
            instances.push(ObsInstance {
                instance_number: instance_number as u32,
                host,
                port: port as u16,
                use_ssl,
                password,
                use_password,
            });
        }
        
        Ok(instances)
    }
    
    async fn get_connection_info(&self, instance_number: u32) -> Result<Option<(bool, Option<DateTime<Utc>>)>, Error> {
        let row_opt = sqlx::query(
            r#"
            SELECT is_connected, last_connected_at
            FROM obs_instances
            WHERE instance_number = $1
            "#
        )
        .bind(instance_number as i32)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            let is_connected: bool = row.get("is_connected");
            let last_connected_at: Option<DateTime<Utc>> = row.get("last_connected_at");
            Ok(Some((is_connected, last_connected_at)))
        } else {
            Ok(None)
        }
    }
}