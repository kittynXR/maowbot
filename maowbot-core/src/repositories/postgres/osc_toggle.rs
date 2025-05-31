use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use maowbot_common::{
    error::Error,
    models::osc_toggle::{OscTrigger, OscToggleState, OscAvatarConfig},
    traits::osc_toggle_traits::OscToggleRepository,
};

pub struct PostgresOscToggleRepository {
    pool: PgPool,
}

impl PostgresOscToggleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OscToggleRepository for PostgresOscToggleRepository {
    async fn get_trigger_by_id(&self, id: i32) -> Result<Option<OscTrigger>, Error> {
        let trigger = sqlx::query_as!(
            OscTrigger,
            r#"
            SELECT 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as "cooldown_seconds!",
                COALESCE(enabled, true) as "enabled!",
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as "created_at!",
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as "updated_at!"
            FROM osc_triggers
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(trigger)
    }
    
    async fn get_trigger_by_redeem_id(&self, redeem_id: Uuid) -> Result<Option<OscTrigger>, Error> {
        let trigger = sqlx::query_as!(
            OscTrigger,
            r#"
            SELECT 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as "cooldown_seconds!",
                COALESCE(enabled, true) as "enabled!",
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as "created_at!",
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as "updated_at!"
            FROM osc_triggers
            WHERE redeem_id = $1 AND enabled = true
            "#,
            redeem_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(trigger)
    }
    
    async fn get_all_triggers(&self) -> Result<Vec<OscTrigger>, Error> {
        let triggers = sqlx::query_as!(
            OscTrigger,
            r#"
            SELECT 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as "cooldown_seconds!",
                COALESCE(enabled, true) as "enabled!",
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as "created_at!",
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as "updated_at!"
            FROM osc_triggers
            ORDER BY redeem_id
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(triggers)
    }
    
    async fn create_trigger(&self, trigger: OscTrigger) -> Result<OscTrigger, Error> {
        let result = sqlx::query_as!(
            OscTrigger,
            r#"
            INSERT INTO osc_triggers 
            (redeem_id, parameter_name, parameter_type, on_value, off_value, duration_seconds, cooldown_seconds, enabled)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as "cooldown_seconds!",
                COALESCE(enabled, true) as "enabled!",
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as "created_at!",
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as "updated_at!"
            "#,
            trigger.redeem_id,
            trigger.parameter_name,
            trigger.parameter_type,
            trigger.on_value,
            trigger.off_value,
            trigger.duration_seconds,
            trigger.cooldown_seconds,
            trigger.enabled
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(result)
    }
    
    async fn update_trigger(&self, trigger: OscTrigger) -> Result<OscTrigger, Error> {
        let result = sqlx::query_as!(
            OscTrigger,
            r#"
            UPDATE osc_triggers
            SET parameter_name = $2, parameter_type = $3, on_value = $4, off_value = $5,
                duration_seconds = $6, cooldown_seconds = $7, enabled = $8, updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as "cooldown_seconds!",
                COALESCE(enabled, true) as "enabled!",
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as "created_at!",
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as "updated_at!"
            "#,
            trigger.id,
            trigger.parameter_name,
            trigger.parameter_type,
            trigger.on_value,
            trigger.off_value,
            trigger.duration_seconds,
            trigger.cooldown_seconds,
            trigger.enabled
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(result)
    }
    
    async fn delete_trigger(&self, id: i32) -> Result<(), Error> {
        sqlx::query!(
            r#"
            DELETE FROM osc_triggers WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(())
    }
    
    async fn get_active_toggles(&self, user_id: Uuid) -> Result<Vec<OscToggleState>, Error> {
        let toggles = sqlx::query_as!(
            OscToggleState,
            r#"
            SELECT 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as "activated_at!",
                expires_at::timestamptz as "expires_at",
                COALESCE(is_active, true) as "is_active!"
            FROM osc_toggle_states
            WHERE user_id = $1 AND is_active = true
            ORDER BY activated_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(toggles)
    }
    
    async fn get_all_active_toggles(&self) -> Result<Vec<OscToggleState>, Error> {
        let toggles = sqlx::query_as!(
            OscToggleState,
            r#"
            SELECT 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as "activated_at!",
                expires_at::timestamptz as "expires_at",
                COALESCE(is_active, true) as "is_active!"
            FROM osc_toggle_states
            WHERE is_active = true
            ORDER BY activated_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(toggles)
    }
    
    async fn get_expired_toggles(&self) -> Result<Vec<OscToggleState>, Error> {
        let toggles = sqlx::query_as!(
            OscToggleState,
            r#"
            SELECT 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as "activated_at!",
                expires_at::timestamptz as "expires_at",
                COALESCE(is_active, true) as "is_active!"
            FROM osc_toggle_states
            WHERE is_active = true AND expires_at IS NOT NULL AND expires_at::timestamptz < CURRENT_TIMESTAMP
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(toggles)
    }
    
    async fn create_toggle_state(&self, state: OscToggleState) -> Result<OscToggleState, Error> {
        // First, deactivate any existing active toggles for this trigger/user
        sqlx::query!(
            r#"
            UPDATE osc_toggle_states
            SET is_active = false
            WHERE trigger_id = $1 AND user_id = $2 AND is_active = true
            "#,
            state.trigger_id,
            state.user_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        // Now create the new active toggle
        let result = sqlx::query_as!(
            OscToggleState,
            r#"
            INSERT INTO osc_toggle_states
            (trigger_id, user_id, avatar_id, expires_at, is_active)
            VALUES ($1, $2, $3, $4, true)
            RETURNING 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as "activated_at!",
                expires_at::timestamptz as "expires_at",
                is_active as "is_active!"
            "#,
            state.trigger_id,
            state.user_id,
            state.avatar_id,
            state.expires_at.map(|dt| dt.naive_utc())
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(result)
    }
    
    async fn deactivate_toggle(&self, id: i32) -> Result<(), Error> {
        sqlx::query!(
            r#"
            UPDATE osc_toggle_states
            SET is_active = false
            WHERE id = $1 AND is_active = true
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(())
    }
    
    async fn cleanup_expired_toggles(&self) -> Result<i64, Error> {
        let result = sqlx::query!(
            r#"
            UPDATE osc_toggle_states
            SET is_active = false
            WHERE is_active = true AND expires_at IS NOT NULL AND expires_at::timestamptz < CURRENT_TIMESTAMP
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(result.rows_affected() as i64)
    }
    
    async fn get_avatar_config(&self, avatar_id: &str) -> Result<Option<OscAvatarConfig>, Error> {
        let config = sqlx::query_as!(
            OscAvatarConfig,
            r#"
            SELECT 
                id,
                avatar_id,
                avatar_name,
                parameter_mappings,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as "created_at!",
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as "updated_at!"
            FROM osc_avatar_configs
            WHERE avatar_id = $1
            "#,
            avatar_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(config)
    }
    
    async fn create_or_update_avatar_config(&self, config: OscAvatarConfig) -> Result<OscAvatarConfig, Error> {
        let result = sqlx::query_as!(
            OscAvatarConfig,
            r#"
            INSERT INTO osc_avatar_configs
            (avatar_id, avatar_name, parameter_mappings)
            VALUES ($1, $2, $3)
            ON CONFLICT (avatar_id) DO UPDATE
            SET avatar_name = EXCLUDED.avatar_name,
                parameter_mappings = EXCLUDED.parameter_mappings,
                updated_at = CURRENT_TIMESTAMP
            RETURNING 
                id,
                avatar_id,
                avatar_name,
                parameter_mappings,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as "created_at!",
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as "updated_at!"
            "#,
            config.avatar_id,
            config.avatar_name,
            config.parameter_mappings
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(result)
    }
}