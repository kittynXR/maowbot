// File: maowbot-core/src/repositories/postgres/ai.rs

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, query, query_as};
use sqlx::postgres::PgQueryResult;
use uuid::Uuid;
use maowbot_common::error::Error;
use maowbot_common::models::ai::{
    AiProvider, AiCredential, AiModel, AiTrigger, AiMemory, AiConfiguration, 
    AiTriggerWithDetails, AiAgent, AiAction, AiSystemPrompt, AiAgentWithDetails
};
use maowbot_common::traits::repository_traits::{
    AiProviderRepository, AiCredentialRepository, AiModelRepository, 
    AiTriggerRepository, AiMemoryRepository, AiConfigurationRepository,
    AiAgentRepository, AiActionRepository, AiSystemPromptRepository
};
use crate::crypto::Encryptor;

pub struct PostgresAiProviderRepository {
    pool: PgPool,
}

impl PostgresAiProviderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiProviderRepository for PostgresAiProviderRepository {
    async fn create_provider(&self, provider: &AiProvider) -> Result<(), Error> {
        query(
            r#"
            INSERT INTO ai_providers (
                provider_id, name, description, enabled, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&provider.provider_id)
        .bind(&provider.name)
        .bind(&provider.description)
        .bind(&provider.enabled)
        .bind(&provider.created_at)
        .bind(&provider.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_provider(&self, provider_id: Uuid) -> Result<Option<AiProvider>, Error> {
        query_as::<_, AiProvider>(
            r#"
            SELECT 
                provider_id, name, description, enabled, created_at, updated_at
            FROM ai_providers
            WHERE provider_id = $1
            "#,
        )
        .bind(&provider_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_provider_by_name(&self, name: &str) -> Result<Option<AiProvider>, Error> {
        query_as::<_, AiProvider>(
            r#"
            SELECT 
                provider_id, name, description, enabled, created_at, updated_at
            FROM ai_providers
            WHERE LOWER(name) = LOWER($1)
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_providers(&self) -> Result<Vec<AiProvider>, Error> {
        query_as::<_, AiProvider>(
            r#"
            SELECT 
                provider_id, name, description, enabled, created_at, updated_at
            FROM ai_providers
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn update_provider(&self, provider: &AiProvider) -> Result<(), Error> {
        query(
            r#"
            UPDATE ai_providers
            SET 
                name = $2, 
                description = $3, 
                enabled = $4, 
                updated_at = $5
            WHERE provider_id = $1
            "#,
        )
        .bind(&provider.provider_id)
        .bind(&provider.name)
        .bind(&provider.description)
        .bind(&provider.enabled)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_provider(&self, provider_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_providers
            WHERE provider_id = $1
            "#,
        )
        .bind(&provider_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

pub struct PostgresAiCredentialRepository {
    pool: PgPool,
    encryptor: Encryptor,
}

impl PostgresAiCredentialRepository {
    pub fn new(pool: PgPool, encryptor: Encryptor) -> Self {
        Self { pool, encryptor }
    }

    async fn encrypt_credentials(&self, credential: &AiCredential) -> Result<AiCredential, Error> {
        let mut encrypted = credential.clone();
        encrypted.api_key = self.encryptor.encrypt(&credential.api_key)?;
        Ok(encrypted)
    }

    async fn decrypt_credentials(&self, credential: &AiCredential) -> Result<AiCredential, Error> {
        let mut decrypted = credential.clone();
        decrypted.api_key = self.encryptor.decrypt(&credential.api_key)?;
        Ok(decrypted)
    }
}

#[async_trait]
impl AiCredentialRepository for PostgresAiCredentialRepository {
    async fn create_credential(&self, credential: &AiCredential) -> Result<(), Error> {
        let encrypted = self.encrypt_credentials(credential).await?;

        query(
            r#"
            INSERT INTO ai_credentials (
                credential_id, provider_id, api_key, api_base, 
                is_default, additional_data, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&encrypted.credential_id)
        .bind(&encrypted.provider_id)
        .bind(&encrypted.api_key)
        .bind(&encrypted.api_base)
        .bind(&encrypted.is_default)
        .bind(&encrypted.additional_data)
        .bind(&encrypted.created_at)
        .bind(&encrypted.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_credential(&self, credential_id: Uuid) -> Result<Option<AiCredential>, Error> {
        let maybe_credential = query_as::<_, AiCredential>(
            r#"
            SELECT 
                credential_id, provider_id, api_key, api_base, 
                is_default, additional_data, created_at, updated_at
            FROM ai_credentials
            WHERE credential_id = $1
            "#,
        )
        .bind(&credential_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        if let Some(cred) = maybe_credential {
            self.decrypt_credentials(&cred).await.map(Some)
        } else {
            Ok(None)
        }
    }

    async fn list_credentials_for_provider(&self, provider_id: Uuid) -> Result<Vec<AiCredential>, Error> {
        let credentials = query_as::<_, AiCredential>(
            r#"
            SELECT 
                credential_id, provider_id, api_key, api_base, 
                is_default, additional_data, created_at, updated_at
            FROM ai_credentials
            WHERE provider_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(&provider_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        let mut decrypted = Vec::with_capacity(credentials.len());
        for cred in credentials {
            decrypted.push(self.decrypt_credentials(&cred).await?);
        }

        Ok(decrypted)
    }

    async fn get_default_credential_for_provider(&self, provider_id: Uuid) -> Result<Option<AiCredential>, Error> {
        let maybe_credential = query_as::<_, AiCredential>(
            r#"
            SELECT 
                credential_id, provider_id, api_key, api_base, 
                is_default, additional_data, created_at, updated_at
            FROM ai_credentials
            WHERE provider_id = $1 AND is_default = true
            "#,
        )
        .bind(&provider_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        if let Some(cred) = maybe_credential {
            self.decrypt_credentials(&cred).await.map(Some)
        } else {
            Ok(None)
        }
    }

    async fn update_credential(&self, credential: &AiCredential) -> Result<(), Error> {
        let encrypted = self.encrypt_credentials(credential).await?;

        query(
            r#"
            UPDATE ai_credentials
            SET 
                api_key = $2, 
                api_base = $3, 
                is_default = $4, 
                additional_data = $5,
                updated_at = $6
            WHERE credential_id = $1
            "#,
        )
        .bind(&encrypted.credential_id)
        .bind(&encrypted.api_key)
        .bind(&encrypted.api_base)
        .bind(&encrypted.is_default)
        .bind(&encrypted.additional_data)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn set_default_credential(&self, credential_id: Uuid) -> Result<(), Error> {
        // Begin transaction
        let mut tx = self.pool.begin().await.map_err(|e| Error::Database(e.to_string()))?;

        // Get provider_id for the credential
        let provider_id: Uuid = query(
            r#"
            SELECT provider_id
            FROM ai_credentials
            WHERE credential_id = $1
            "#,
        )
        .bind(&credential_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

        // Clear existing default for this provider
        query(
            r#"
            UPDATE ai_credentials
            SET is_default = false, updated_at = $2
            WHERE provider_id = $1 AND is_default = true
            "#,
        )
        .bind(&provider_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Set new default
        query(
            r#"
            UPDATE ai_credentials
            SET is_default = true, updated_at = $2
            WHERE credential_id = $1
            "#,
        )
        .bind(&credential_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Commit transaction
        tx.commit().await.map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_credential(&self, credential_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_credentials
            WHERE credential_id = $1
            "#,
        )
        .bind(&credential_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

pub struct PostgresAiModelRepository {
    pool: PgPool,
}

impl PostgresAiModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiModelRepository for PostgresAiModelRepository {
    async fn create_model(&self, model: &AiModel) -> Result<(), Error> {
        query(
            r#"
            INSERT INTO ai_models (
                model_id, provider_id, name, description, 
                is_default, capabilities, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&model.model_id)
        .bind(&model.provider_id)
        .bind(&model.name)
        .bind(&model.description)
        .bind(&model.is_default)
        .bind(&model.capabilities)
        .bind(&model.created_at)
        .bind(&model.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_model(&self, model_id: Uuid) -> Result<Option<AiModel>, Error> {
        query_as::<_, AiModel>(
            r#"
            SELECT 
                model_id, provider_id, name, description, 
                is_default, capabilities, created_at, updated_at
            FROM ai_models
            WHERE model_id = $1
            "#,
        )
        .bind(&model_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_model_by_name(&self, provider_id: Uuid, name: &str) -> Result<Option<AiModel>, Error> {
        query_as::<_, AiModel>(
            r#"
            SELECT 
                model_id, provider_id, name, description, 
                is_default, capabilities, created_at, updated_at
            FROM ai_models
            WHERE provider_id = $1 AND LOWER(name) = LOWER($2)
            "#,
        )
        .bind(&provider_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_models_for_provider(&self, provider_id: Uuid) -> Result<Vec<AiModel>, Error> {
        query_as::<_, AiModel>(
            r#"
            SELECT 
                model_id, provider_id, name, description, 
                is_default, capabilities, created_at, updated_at
            FROM ai_models
            WHERE provider_id = $1
            ORDER BY name
            "#,
        )
        .bind(&provider_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_default_model_for_provider(&self, provider_id: Uuid) -> Result<Option<AiModel>, Error> {
        query_as::<_, AiModel>(
            r#"
            SELECT 
                model_id, provider_id, name, description, 
                is_default, capabilities, created_at, updated_at
            FROM ai_models
            WHERE provider_id = $1 AND is_default = true
            "#,
        )
        .bind(&provider_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn update_model(&self, model: &AiModel) -> Result<(), Error> {
        query(
            r#"
            UPDATE ai_models
            SET 
                name = $2, 
                description = $3, 
                is_default = $4, 
                capabilities = $5,
                updated_at = $6
            WHERE model_id = $1
            "#,
        )
        .bind(&model.model_id)
        .bind(&model.name)
        .bind(&model.description)
        .bind(&model.is_default)
        .bind(&model.capabilities)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn set_default_model(&self, model_id: Uuid) -> Result<(), Error> {
        // Begin transaction
        let mut tx = self.pool.begin().await.map_err(|e| Error::Database(e.to_string()))?;

        // Get provider_id for the model
        let provider_id: Uuid = query(
            r#"
            SELECT provider_id
            FROM ai_models
            WHERE model_id = $1
            "#,
        )
        .bind(&model_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .get(0);

        // Clear existing default for this provider
        query(
            r#"
            UPDATE ai_models
            SET is_default = false, updated_at = $2
            WHERE provider_id = $1 AND is_default = true
            "#,
        )
        .bind(&provider_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Set new default
        query(
            r#"
            UPDATE ai_models
            SET is_default = true, updated_at = $2
            WHERE model_id = $1
            "#,
        )
        .bind(&model_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Commit transaction
        tx.commit().await.map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_model(&self, model_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_models
            WHERE model_id = $1
            "#,
        )
        .bind(&model_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

pub struct PostgresAiTriggerRepository {
    pool: PgPool,
}

impl PostgresAiTriggerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiTriggerRepository for PostgresAiTriggerRepository {
    async fn create_trigger(&self, trigger: &AiTrigger) -> Result<(), Error> {
        query(
            r#"
            INSERT INTO ai_triggers (
                trigger_id, trigger_type, pattern, model_id, agent_id,
                system_prompt, platform, channel, schedule, condition,
                enabled, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(&trigger.trigger_id)
        .bind(&trigger.trigger_type)
        .bind(&trigger.pattern)
        .bind(&trigger.model_id)
        .bind(&trigger.agent_id)
        .bind(&trigger.system_prompt)
        .bind(&trigger.platform)
        .bind(&trigger.channel)
        .bind(&trigger.schedule)
        .bind(&trigger.condition)
        .bind(&trigger.enabled)
        .bind(&trigger.created_at)
        .bind(&trigger.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_trigger(&self, trigger_id: Uuid) -> Result<Option<AiTrigger>, Error> {
        query_as::<_, AiTrigger>(
            r#"
            SELECT 
                trigger_id, trigger_type, pattern, model_id, agent_id,
                system_prompt, platform, channel, schedule, condition,
                enabled, created_at, updated_at
            FROM ai_triggers
            WHERE trigger_id = $1
            "#,
        )
        .bind(&trigger_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_trigger_by_pattern(&self, pattern: &str) -> Result<Option<AiTrigger>, Error> {
        query_as::<_, AiTrigger>(
            r#"
            SELECT 
                trigger_id, trigger_type, pattern, model_id, agent_id,
                system_prompt, platform, channel, schedule, condition,
                enabled, created_at, updated_at
            FROM ai_triggers
            WHERE LOWER(pattern) = LOWER($1)
            "#,
        )
        .bind(pattern)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_triggers(&self) -> Result<Vec<AiTrigger>, Error> {
        query_as::<_, AiTrigger>(
            r#"
            SELECT 
                trigger_id, trigger_type, pattern, model_id, agent_id,
                system_prompt, platform, channel, schedule, condition,
                enabled, created_at, updated_at
            FROM ai_triggers
            ORDER BY pattern
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_triggers_for_model(&self, model_id: Uuid) -> Result<Vec<AiTrigger>, Error> {
        query_as::<_, AiTrigger>(
            r#"
            SELECT 
                trigger_id, trigger_type, pattern, model_id, agent_id,
                system_prompt, platform, channel, schedule, condition,
                enabled, created_at, updated_at
            FROM ai_triggers
            WHERE model_id = $1
            ORDER BY pattern
            "#,
        )
        .bind(&model_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_triggers_for_agent(&self, agent_id: Uuid) -> Result<Vec<AiTrigger>, Error> {
        query_as::<_, AiTrigger>(
            r#"
            SELECT 
                trigger_id, trigger_type, pattern, model_id, agent_id,
                system_prompt, platform, channel, schedule, condition,
                enabled, created_at, updated_at
            FROM ai_triggers
            WHERE agent_id = $1
            ORDER BY pattern
            "#,
        )
        .bind(&agent_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_triggers_with_details(&self) -> Result<Vec<AiTriggerWithDetails>, Error> {
        #[derive(sqlx::FromRow)]
        struct JoinedTrigger {
            // Trigger fields
            trigger_id: Uuid,
            trigger_type: String,
            pattern: String,
            model_id: Option<Uuid>,
            agent_id: Option<Uuid>,
            system_prompt: Option<String>,
            platform: Option<String>,
            channel: Option<String>,
            schedule: Option<String>,
            condition: Option<String>,
            enabled: bool,
            trigger_created_at: DateTime<Utc>,
            trigger_updated_at: DateTime<Utc>,
            
            // Model fields (optional)
            model_name: Option<String>,
            model_description: Option<String>,
            model_is_default: Option<bool>,
            model_capabilities: Option<sqlx::types::Json<serde_json::Value>>,
            model_created_at: Option<DateTime<Utc>>,
            model_updated_at: Option<DateTime<Utc>>,
            
            // Provider fields (optional)
            provider_id: Option<Uuid>,
            provider_name: Option<String>,
            provider_description: Option<String>,
            provider_enabled: Option<bool>,
            provider_created_at: Option<DateTime<Utc>>,
            provider_updated_at: Option<DateTime<Utc>>,
            
            // Agent fields (optional)
            agent_name: Option<String>,
            agent_description: Option<String>,
            agent_system_prompt: Option<String>,
            agent_capabilities: Option<sqlx::types::Json<serde_json::Value>>,
            agent_enabled: Option<bool>,
            agent_created_at: Option<DateTime<Utc>>,
            agent_updated_at: Option<DateTime<Utc>>,
        }

        let joined_triggers = query_as::<_, JoinedTrigger>(
            r#"
            SELECT 
                t.trigger_id, t.trigger_type, t.pattern, t.model_id, t.agent_id,
                t.system_prompt, t.platform, t.channel, t.schedule, t.condition,
                t.enabled, 
                t.created_at as trigger_created_at, 
                t.updated_at as trigger_updated_at,
                
                m.name as model_name, 
                m.description as model_description, 
                m.is_default as model_is_default, 
                m.capabilities as model_capabilities,
                m.created_at as model_created_at, 
                m.updated_at as model_updated_at,
                
                p.provider_id, p.name as provider_name, 
                p.description as provider_description, 
                p.enabled as provider_enabled,
                p.created_at as provider_created_at, 
                p.updated_at as provider_updated_at,
                
                a.name as agent_name,
                a.description as agent_description,
                a.system_prompt as agent_system_prompt,
                a.capabilities as agent_capabilities,
                a.enabled as agent_enabled,
                a.created_at as agent_created_at,
                a.updated_at as agent_updated_at
            FROM ai_triggers t
            LEFT JOIN ai_models m ON t.model_id = m.model_id
            LEFT JOIN ai_providers p ON m.provider_id = p.provider_id
            LEFT JOIN ai_agents a ON t.agent_id = a.agent_id
            ORDER BY t.pattern
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        let result = joined_triggers.into_iter().map(|jt| {
            let trigger = AiTrigger {
                trigger_id: jt.trigger_id,
                trigger_type: jt.trigger_type,
                pattern: jt.pattern,
                model_id: jt.model_id,
                agent_id: jt.agent_id,
                system_prompt: jt.system_prompt,
                platform: jt.platform,
                channel: jt.channel,
                schedule: jt.schedule,
                condition: jt.condition,
                enabled: jt.enabled,
                created_at: jt.trigger_created_at,
                updated_at: jt.trigger_updated_at,
            };
            
            // Create model if model fields are present
            let model = if jt.model_id.is_some() {
                Some(AiModel {
                    model_id: jt.model_id.unwrap(),
                    provider_id: jt.provider_id.unwrap_or_else(Uuid::nil),
                    name: jt.model_name.unwrap_or_default(),
                    description: jt.model_description,
                    is_default: jt.model_is_default.unwrap_or(false),
                    capabilities: jt.model_capabilities.map(|j| j.0),
                    created_at: jt.model_created_at.unwrap_or_else(Utc::now),
                    updated_at: jt.model_updated_at.unwrap_or_else(Utc::now),
                })
            } else {
                None
            };
            
            // Create provider if provider fields are present
            let provider = if jt.provider_id.is_some() {
                Some(AiProvider {
                    provider_id: jt.provider_id.unwrap(),
                    name: jt.provider_name.unwrap_or_default(),
                    description: jt.provider_description,
                    enabled: jt.provider_enabled.unwrap_or(true),
                    created_at: jt.provider_created_at.unwrap_or_else(Utc::now),
                    updated_at: jt.provider_updated_at.unwrap_or_else(Utc::now),
                })
            } else {
                None
            };
            
            // Create agent if agent fields are present
            let agent = if jt.agent_id.is_some() {
                Some(AiAgent {
                    agent_id: jt.agent_id.unwrap(),
                    name: jt.agent_name.unwrap_or_default(),
                    description: jt.agent_description,
                    model_id: jt.model_id.unwrap_or_else(Uuid::nil),
                    system_prompt: jt.agent_system_prompt,
                    capabilities: jt.agent_capabilities.map(|j| j.0),
                    enabled: jt.agent_enabled.unwrap_or(true),
                    created_at: jt.agent_created_at.unwrap_or_else(Utc::now),
                    updated_at: jt.agent_updated_at.unwrap_or_else(Utc::now),
                })
            } else {
                None
            };
            
            AiTriggerWithDetails {
                trigger,
                model,
                agent,
                provider,
            }
        }).collect();

        Ok(result)
    }

    async fn update_trigger(&self, trigger: &AiTrigger) -> Result<(), Error> {
        query(
            r#"
            UPDATE ai_triggers
            SET 
                trigger_type = $2, 
                pattern = $3, 
                model_id = $4, 
                agent_id = $5,
                system_prompt = $6,
                platform = $7,
                channel = $8,
                schedule = $9,
                condition = $10,
                enabled = $11,
                updated_at = $12
            WHERE trigger_id = $1
            "#,
        )
        .bind(&trigger.trigger_id)
        .bind(&trigger.trigger_type)
        .bind(&trigger.pattern)
        .bind(&trigger.model_id)
        .bind(&trigger.agent_id)
        .bind(&trigger.system_prompt)
        .bind(&trigger.platform)
        .bind(&trigger.channel)
        .bind(&trigger.schedule)
        .bind(&trigger.condition)
        .bind(&trigger.enabled)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_trigger(&self, trigger_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_triggers
            WHERE trigger_id = $1
            "#,
        )
        .bind(&trigger_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

pub struct PostgresAiMemoryRepository {
    pool: PgPool,
}

impl PostgresAiMemoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiMemoryRepository for PostgresAiMemoryRepository {
    async fn create_memory(&self, memory: &AiMemory) -> Result<(), Error> {
        query(
            r#"
            INSERT INTO ai_memory (
                memory_id, user_id, platform, role, content, timestamp, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&memory.memory_id)
        .bind(&memory.user_id)
        .bind(&memory.platform)
        .bind(&memory.role)
        .bind(&memory.content)
        .bind(&memory.timestamp)
        .bind(&memory.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_memory(&self, memory_id: Uuid) -> Result<Option<AiMemory>, Error> {
        query_as::<_, AiMemory>(
            r#"
            SELECT 
                memory_id, user_id, platform, role, content, timestamp, metadata
            FROM ai_memory
            WHERE memory_id = $1
            "#,
        )
        .bind(&memory_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_memories_for_user(&self, user_id: Uuid, limit: i64) -> Result<Vec<AiMemory>, Error> {
        query_as::<_, AiMemory>(
            r#"
            SELECT 
                memory_id, user_id, platform, role, content, timestamp, metadata
            FROM ai_memory
            WHERE user_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(&user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn delete_memory(&self, memory_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_memory
            WHERE memory_id = $1
            "#,
        )
        .bind(&memory_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_user_memories(&self, user_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_memory
            WHERE user_id = $1
            "#,
        )
        .bind(&user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_old_memories(&self, older_than: DateTime<Utc>) -> Result<i64, Error> {
        let result: PgQueryResult = query(
            r#"
            DELETE FROM ai_memory
            WHERE timestamp < $1
            "#,
        )
        .bind(&older_than)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(result.rows_affected() as i64)
    }
}

pub struct PostgresAiAgentRepository {
    pool: PgPool,
}

impl PostgresAiAgentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiAgentRepository for PostgresAiAgentRepository {
    async fn create_agent(&self, agent: &AiAgent) -> Result<(), Error> {
        query(
            r#"
            INSERT INTO ai_agents (
                agent_id, name, description, model_id, 
                system_prompt, capabilities, enabled, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&agent.agent_id)
        .bind(&agent.name)
        .bind(&agent.description)
        .bind(&agent.model_id)
        .bind(&agent.system_prompt)
        .bind(&agent.capabilities)
        .bind(&agent.enabled)
        .bind(&agent.created_at)
        .bind(&agent.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_agent(&self, agent_id: Uuid) -> Result<Option<AiAgent>, Error> {
        query_as::<_, AiAgent>(
            r#"
            SELECT 
                agent_id, name, description, model_id, 
                system_prompt, capabilities, enabled, created_at, updated_at
            FROM ai_agents
            WHERE agent_id = $1
            "#,
        )
        .bind(&agent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_agent_by_name(&self, name: &str) -> Result<Option<AiAgent>, Error> {
        query_as::<_, AiAgent>(
            r#"
            SELECT 
                agent_id, name, description, model_id, 
                system_prompt, capabilities, enabled, created_at, updated_at
            FROM ai_agents
            WHERE LOWER(name) = LOWER($1)
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_agents(&self) -> Result<Vec<AiAgent>, Error> {
        query_as::<_, AiAgent>(
            r#"
            SELECT 
                agent_id, name, description, model_id, 
                system_prompt, capabilities, enabled, created_at, updated_at
            FROM ai_agents
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_agent_with_details(&self, agent_id: Uuid) -> Result<Option<AiAgentWithDetails>, Error> {
        // Get the basic agent info
        let maybe_agent = self.get_agent(agent_id).await?;
        
        if let Some(agent) = maybe_agent {
            // Get the model info
            let maybe_model = query_as::<_, AiModel>(
                r#"
                SELECT 
                    model_id, provider_id, name, description, 
                    is_default, capabilities, created_at, updated_at
                FROM ai_models
                WHERE model_id = $1
                "#,
            )
            .bind(&agent.model_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;
            
            if let Some(model) = maybe_model {
                // Get the provider info
                let maybe_provider = query_as::<_, AiProvider>(
                    r#"
                    SELECT 
                        provider_id, name, description, enabled, created_at, updated_at
                    FROM ai_providers
                    WHERE provider_id = $1
                    "#,
                )
                .bind(&model.provider_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
                
                if let Some(provider) = maybe_provider {
                    // Get all actions for this agent
                    let actions = query_as::<_, AiAction>(
                        r#"
                        SELECT 
                            action_id, agent_id, name, description, 
                            input_schema, output_schema, handler_type, handler_config,
                            enabled, created_at, updated_at
                        FROM ai_actions
                        WHERE agent_id = $1
                        "#,
                    )
                    .bind(&agent.agent_id)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?;
                    
                    return Ok(Some(AiAgentWithDetails {
                        agent,
                        model,
                        provider,
                        actions,
                    }));
                }
            }
        }
        
        Ok(None)
    }

    async fn update_agent(&self, agent: &AiAgent) -> Result<(), Error> {
        query(
            r#"
            UPDATE ai_agents
            SET 
                name = $2, 
                description = $3, 
                model_id = $4, 
                system_prompt = $5,
                capabilities = $6,
                enabled = $7,
                updated_at = $8
            WHERE agent_id = $1
            "#,
        )
        .bind(&agent.agent_id)
        .bind(&agent.name)
        .bind(&agent.description)
        .bind(&agent.model_id)
        .bind(&agent.system_prompt)
        .bind(&agent.capabilities)
        .bind(&agent.enabled)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_agent(&self, agent_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_agents
            WHERE agent_id = $1
            "#,
        )
        .bind(&agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

pub struct PostgresAiActionRepository {
    pool: PgPool,
}

impl PostgresAiActionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiActionRepository for PostgresAiActionRepository {
    async fn create_action(&self, action: &AiAction) -> Result<(), Error> {
        query(
            r#"
            INSERT INTO ai_actions (
                action_id, agent_id, name, description, 
                input_schema, output_schema, handler_type, handler_config,
                enabled, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(&action.action_id)
        .bind(&action.agent_id)
        .bind(&action.name)
        .bind(&action.description)
        .bind(&action.input_schema)
        .bind(&action.output_schema)
        .bind(&action.handler_type)
        .bind(&action.handler_config)
        .bind(&action.enabled)
        .bind(&action.created_at)
        .bind(&action.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_action(&self, action_id: Uuid) -> Result<Option<AiAction>, Error> {
        query_as::<_, AiAction>(
            r#"
            SELECT 
                action_id, agent_id, name, description, 
                input_schema, output_schema, handler_type, handler_config,
                enabled, created_at, updated_at
            FROM ai_actions
            WHERE action_id = $1
            "#,
        )
        .bind(&action_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_action_by_name(&self, agent_id: Uuid, name: &str) -> Result<Option<AiAction>, Error> {
        query_as::<_, AiAction>(
            r#"
            SELECT 
                action_id, agent_id, name, description, 
                input_schema, output_schema, handler_type, handler_config,
                enabled, created_at, updated_at
            FROM ai_actions
            WHERE agent_id = $1 AND LOWER(name) = LOWER($2)
            "#,
        )
        .bind(&agent_id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_actions_for_agent(&self, agent_id: Uuid) -> Result<Vec<AiAction>, Error> {
        query_as::<_, AiAction>(
            r#"
            SELECT 
                action_id, agent_id, name, description, 
                input_schema, output_schema, handler_type, handler_config,
                enabled, created_at, updated_at
            FROM ai_actions
            WHERE agent_id = $1
            ORDER BY name
            "#,
        )
        .bind(&agent_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn update_action(&self, action: &AiAction) -> Result<(), Error> {
        query(
            r#"
            UPDATE ai_actions
            SET 
                name = $2, 
                description = $3, 
                input_schema = $4, 
                output_schema = $5,
                handler_type = $6,
                handler_config = $7,
                enabled = $8,
                updated_at = $9
            WHERE action_id = $1
            "#,
        )
        .bind(&action.action_id)
        .bind(&action.name)
        .bind(&action.description)
        .bind(&action.input_schema)
        .bind(&action.output_schema)
        .bind(&action.handler_type)
        .bind(&action.handler_config)
        .bind(&action.enabled)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_action(&self, action_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_actions
            WHERE action_id = $1
            "#,
        )
        .bind(&action_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

pub struct PostgresAiSystemPromptRepository {
    pool: PgPool,
}

impl PostgresAiSystemPromptRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AiSystemPromptRepository for PostgresAiSystemPromptRepository {
    async fn create_prompt(&self, prompt: &AiSystemPrompt) -> Result<(), Error> {
        query(
            r#"
            INSERT INTO ai_system_prompts (
                prompt_id, name, content, description, 
                is_default, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&prompt.prompt_id)
        .bind(&prompt.name)
        .bind(&prompt.content)
        .bind(&prompt.description)
        .bind(&prompt.is_default)
        .bind(&prompt.created_at)
        .bind(&prompt.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_prompt(&self, prompt_id: Uuid) -> Result<Option<AiSystemPrompt>, Error> {
        query_as::<_, AiSystemPrompt>(
            r#"
            SELECT 
                prompt_id, name, content, description, 
                is_default, created_at, updated_at
            FROM ai_system_prompts
            WHERE prompt_id = $1
            "#,
        )
        .bind(&prompt_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_prompt_by_name(&self, name: &str) -> Result<Option<AiSystemPrompt>, Error> {
        query_as::<_, AiSystemPrompt>(
            r#"
            SELECT 
                prompt_id, name, content, description, 
                is_default, created_at, updated_at
            FROM ai_system_prompts
            WHERE LOWER(name) = LOWER($1)
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn get_default_prompt(&self) -> Result<Option<AiSystemPrompt>, Error> {
        query_as::<_, AiSystemPrompt>(
            r#"
            SELECT 
                prompt_id, name, content, description, 
                is_default, created_at, updated_at
            FROM ai_system_prompts
            WHERE is_default = true
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn list_prompts(&self) -> Result<Vec<AiSystemPrompt>, Error> {
        query_as::<_, AiSystemPrompt>(
            r#"
            SELECT 
                prompt_id, name, content, description, 
                is_default, created_at, updated_at
            FROM ai_system_prompts
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))
    }

    async fn update_prompt(&self, prompt: &AiSystemPrompt) -> Result<(), Error> {
        query(
            r#"
            UPDATE ai_system_prompts
            SET 
                name = $2, 
                content = $3, 
                description = $4, 
                is_default = $5,
                updated_at = $6
            WHERE prompt_id = $1
            "#,
        )
        .bind(&prompt.prompt_id)
        .bind(&prompt.name)
        .bind(&prompt.content)
        .bind(&prompt.description)
        .bind(&prompt.is_default)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn set_default_prompt(&self, prompt_id: Uuid) -> Result<(), Error> {
        // Begin transaction
        let mut tx = self.pool.begin().await.map_err(|e| Error::Database(e.to_string()))?;

        // Clear existing default
        query(
            r#"
            UPDATE ai_system_prompts
            SET is_default = false, updated_at = $1
            WHERE is_default = true
            "#,
        )
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Set new default
        query(
            r#"
            UPDATE ai_system_prompts
            SET is_default = true, updated_at = $2
            WHERE prompt_id = $1
            "#,
        )
        .bind(&prompt_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        // Commit transaction
        tx.commit().await.map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_prompt(&self, prompt_id: Uuid) -> Result<(), Error> {
        query(
            r#"
            DELETE FROM ai_system_prompts
            WHERE prompt_id = $1
            "#,
        )
        .bind(&prompt_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

pub struct PostgresAiConfigurationRepository {
    pool: PgPool,
    encryptor: Encryptor,
}

impl PostgresAiConfigurationRepository {
    pub fn new(pool: PgPool, encryptor: Encryptor) -> Self {
        Self { pool, encryptor }
    }
}

#[async_trait]
impl AiConfigurationRepository for PostgresAiConfigurationRepository {
    async fn get_default_configuration(&self) -> Result<Option<AiConfiguration>, Error> {
        // Get a default provider that's enabled
        let maybe_provider = query_as::<_, AiProvider>(
            r#"
            SELECT provider_id, name, description, enabled, created_at, updated_at
            FROM ai_providers
            WHERE enabled = true
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        if let Some(provider) = maybe_provider {
            // Get default credential for the provider
            let maybe_credential = query_as::<_, AiCredential>(
                r#"
                SELECT credential_id, provider_id, api_key, api_base, 
                    is_default, additional_data, created_at, updated_at
                FROM ai_credentials
                WHERE provider_id = $1 AND is_default = true
                "#,
            )
            .bind(&provider.provider_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

            if let Some(credential) = maybe_credential {
                // Decrypt the credential
                let decrypted_credential = AiCredential {
                    api_key: self.encryptor.decrypt(&credential.api_key)?,
                    ..credential
                };

                // Get default model for the provider
                let maybe_model = query_as::<_, AiModel>(
                    r#"
                    SELECT model_id, provider_id, name, description, 
                        is_default, capabilities, created_at, updated_at
                    FROM ai_models
                    WHERE provider_id = $1 AND is_default = true
                    "#,
                )
                .bind(&provider.provider_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;

                if let Some(model) = maybe_model {
                    return Ok(Some(AiConfiguration {
                        provider,
                        credential: decrypted_credential,
                        model,
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn get_configuration_for_provider(&self, provider_name: &str) -> Result<Option<AiConfiguration>, Error> {
        // Get provider by name (if enabled)
        let maybe_provider = query_as::<_, AiProvider>(
            r#"
            SELECT provider_id, name, description, enabled, created_at, updated_at
            FROM ai_providers
            WHERE LOWER(name) = LOWER($1) AND enabled = true
            "#,
        )
        .bind(provider_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

        if let Some(provider) = maybe_provider {
            // Get default credential for the provider
            let maybe_credential = query_as::<_, AiCredential>(
                r#"
                SELECT credential_id, provider_id, api_key, api_base, 
                    is_default, additional_data, created_at, updated_at
                FROM ai_credentials
                WHERE provider_id = $1 AND is_default = true
                "#,
            )
            .bind(&provider.provider_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

            if let Some(credential) = maybe_credential {
                // Decrypt the credential
                let decrypted_credential = AiCredential {
                    api_key: self.encryptor.decrypt(&credential.api_key)?,
                    ..credential
                };

                // Get default model for the provider
                let maybe_model = query_as::<_, AiModel>(
                    r#"
                    SELECT model_id, provider_id, name, description, 
                        is_default, capabilities, created_at, updated_at
                    FROM ai_models
                    WHERE provider_id = $1 AND is_default = true
                    "#,
                )
                .bind(&provider.provider_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;

                if let Some(model) = maybe_model {
                    return Ok(Some(AiConfiguration {
                        provider,
                        credential: decrypted_credential,
                        model,
                    }));
                }
            }
        }

        Ok(None)
    }
}