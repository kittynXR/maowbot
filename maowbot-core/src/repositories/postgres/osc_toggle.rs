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
        let row = sqlx::query(
            r#"
            SELECT 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as cooldown_seconds,
                COALESCE(enabled, true) as enabled,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as created_at,
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as updated_at
            FROM osc_triggers
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        if let Some(r) = row {
            let trigger = OscTrigger {
                id: r.try_get("id")?,
                redeem_id: r.try_get("redeem_id")?,
                parameter_name: r.try_get("parameter_name")?,
                parameter_type: r.try_get("parameter_type")?,
                on_value: r.try_get("on_value")?,
                off_value: r.try_get("off_value")?,
                duration_seconds: r.try_get("duration_seconds")?,
                cooldown_seconds: r.try_get("cooldown_seconds")?,
                enabled: r.try_get("enabled")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            };
            Ok(Some(trigger))
        } else {
            Ok(None)
        }
    }
    
    async fn get_trigger_by_redeem_id(&self, redeem_id: Uuid) -> Result<Option<OscTrigger>, Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as cooldown_seconds,
                COALESCE(enabled, true) as enabled,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as created_at,
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as updated_at
            FROM osc_triggers
            WHERE redeem_id = $1 AND enabled = true
            "#,
        )
        .bind(redeem_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        if let Some(r) = row {
            let trigger = OscTrigger {
                id: r.try_get("id")?,
                redeem_id: r.try_get("redeem_id")?,
                parameter_name: r.try_get("parameter_name")?,
                parameter_type: r.try_get("parameter_type")?,
                on_value: r.try_get("on_value")?,
                off_value: r.try_get("off_value")?,
                duration_seconds: r.try_get("duration_seconds")?,
                cooldown_seconds: r.try_get("cooldown_seconds")?,
                enabled: r.try_get("enabled")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            };
            Ok(Some(trigger))
        } else {
            Ok(None)
        }
    }
    
    async fn get_all_triggers(&self) -> Result<Vec<OscTrigger>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                redeem_id,
                parameter_name,
                parameter_type,
                on_value,
                off_value,
                duration_seconds,
                COALESCE(cooldown_seconds, 0) as cooldown_seconds,
                COALESCE(enabled, true) as enabled,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as created_at,
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as updated_at
            FROM osc_triggers
            ORDER BY redeem_id
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let mut triggers = Vec::new();
        for r in rows {
            let trigger = OscTrigger {
                id: r.try_get("id")?,
                redeem_id: r.try_get("redeem_id")?,
                parameter_name: r.try_get("parameter_name")?,
                parameter_type: r.try_get("parameter_type")?,
                on_value: r.try_get("on_value")?,
                off_value: r.try_get("off_value")?,
                duration_seconds: r.try_get("duration_seconds")?,
                cooldown_seconds: r.try_get("cooldown_seconds")?,
                enabled: r.try_get("enabled")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            };
            triggers.push(trigger);
        }
        
        Ok(triggers)
    }
    
    async fn create_trigger(&self, trigger: OscTrigger) -> Result<OscTrigger, Error> {
        let row = sqlx::query(
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
                COALESCE(cooldown_seconds, 0) as cooldown_seconds,
                COALESCE(enabled, true) as enabled,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as created_at,
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as updated_at
            "#,
        )
        .bind(trigger.redeem_id)
        .bind(&trigger.parameter_name)
        .bind(&trigger.parameter_type)
        .bind(&trigger.on_value)
        .bind(&trigger.off_value)
        .bind(trigger.duration_seconds)
        .bind(trigger.cooldown_seconds)
        .bind(trigger.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let result = OscTrigger {
            id: row.try_get("id")?,
            redeem_id: row.try_get("redeem_id")?,
            parameter_name: row.try_get("parameter_name")?,
            parameter_type: row.try_get("parameter_type")?,
            on_value: row.try_get("on_value")?,
            off_value: row.try_get("off_value")?,
            duration_seconds: row.try_get("duration_seconds")?,
            cooldown_seconds: row.try_get("cooldown_seconds")?,
            enabled: row.try_get("enabled")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        
        Ok(result)
    }
    
    async fn update_trigger(&self, trigger: OscTrigger) -> Result<OscTrigger, Error> {
        let row = sqlx::query(
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
                COALESCE(cooldown_seconds, 0) as cooldown_seconds,
                COALESCE(enabled, true) as enabled,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as created_at,
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as updated_at
            "#,
        )
        .bind(trigger.id)
        .bind(&trigger.parameter_name)
        .bind(&trigger.parameter_type)
        .bind(&trigger.on_value)
        .bind(&trigger.off_value)
        .bind(trigger.duration_seconds)
        .bind(trigger.cooldown_seconds)
        .bind(trigger.enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let result = OscTrigger {
            id: row.try_get("id")?,
            redeem_id: row.try_get("redeem_id")?,
            parameter_name: row.try_get("parameter_name")?,
            parameter_type: row.try_get("parameter_type")?,
            on_value: row.try_get("on_value")?,
            off_value: row.try_get("off_value")?,
            duration_seconds: row.try_get("duration_seconds")?,
            cooldown_seconds: row.try_get("cooldown_seconds")?,
            enabled: row.try_get("enabled")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        
        Ok(result)
    }
    
    async fn delete_trigger(&self, id: i32) -> Result<(), Error> {
        sqlx::query(
            r#"
            DELETE FROM osc_triggers WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(())
    }
    
    async fn get_active_toggles(&self, user_id: Uuid) -> Result<Vec<OscToggleState>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as activated_at,
                expires_at::timestamptz as expires_at,
                COALESCE(is_active, true) as is_active
            FROM osc_toggle_states
            WHERE user_id = $1 AND is_active = true
            ORDER BY activated_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let mut toggles = Vec::new();
        for r in rows {
            let toggle = OscToggleState {
                id: r.try_get("id")?,
                trigger_id: r.try_get("trigger_id")?,
                user_id: r.try_get("user_id")?,
                avatar_id: r.try_get("avatar_id")?,
                activated_at: r.try_get("activated_at")?,
                expires_at: r.try_get("expires_at")?,
                is_active: r.try_get("is_active")?,
            };
            toggles.push(toggle);
        }
        
        Ok(toggles)
    }
    
    async fn get_all_active_toggles(&self) -> Result<Vec<OscToggleState>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as activated_at,
                expires_at::timestamptz as expires_at,
                COALESCE(is_active, true) as is_active
            FROM osc_toggle_states
            WHERE is_active = true
            ORDER BY activated_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let mut toggles = Vec::new();
        for r in rows {
            let toggle = OscToggleState {
                id: r.try_get("id")?,
                trigger_id: r.try_get("trigger_id")?,
                user_id: r.try_get("user_id")?,
                avatar_id: r.try_get("avatar_id")?,
                activated_at: r.try_get("activated_at")?,
                expires_at: r.try_get("expires_at")?,
                is_active: r.try_get("is_active")?,
            };
            toggles.push(toggle);
        }
        
        Ok(toggles)
    }
    
    async fn get_expired_toggles(&self) -> Result<Vec<OscToggleState>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as activated_at,
                expires_at::timestamptz as expires_at,
                COALESCE(is_active, true) as is_active
            FROM osc_toggle_states
            WHERE is_active = true AND expires_at IS NOT NULL AND expires_at::timestamptz < CURRENT_TIMESTAMP
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let mut toggles = Vec::new();
        for r in rows {
            let toggle = OscToggleState {
                id: r.try_get("id")?,
                trigger_id: r.try_get("trigger_id")?,
                user_id: r.try_get("user_id")?,
                avatar_id: r.try_get("avatar_id")?,
                activated_at: r.try_get("activated_at")?,
                expires_at: r.try_get("expires_at")?,
                is_active: r.try_get("is_active")?,
            };
            toggles.push(toggle);
        }
        
        Ok(toggles)
    }
    
    async fn create_toggle_state(&self, state: OscToggleState) -> Result<OscToggleState, Error> {
        // First, deactivate any existing active toggles for this trigger/user
        sqlx::query(
            r#"
            UPDATE osc_toggle_states
            SET is_active = false
            WHERE trigger_id = $1 AND user_id = $2 AND is_active = true
            "#,
        )
        .bind(state.trigger_id)
        .bind(state.user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        // Now create the new active toggle
        let row = sqlx::query(
            r#"
            INSERT INTO osc_toggle_states
            (trigger_id, user_id, avatar_id, expires_at, is_active)
            VALUES ($1, $2, $3, $4, true)
            RETURNING 
                id,
                trigger_id,
                user_id,
                avatar_id,
                COALESCE(activated_at, CURRENT_TIMESTAMP)::timestamptz as activated_at,
                expires_at::timestamptz as expires_at,
                is_active
            "#,
        )
        .bind(state.trigger_id)
        .bind(state.user_id)
        .bind(&state.avatar_id)
        .bind(state.expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let result = OscToggleState {
            id: row.try_get("id")?,
            trigger_id: row.try_get("trigger_id")?,
            user_id: row.try_get("user_id")?,
            avatar_id: row.try_get("avatar_id")?,
            activated_at: row.try_get("activated_at")?,
            expires_at: row.try_get("expires_at")?,
            is_active: row.try_get("is_active")?,
        };
        
        Ok(result)
    }
    
    async fn deactivate_toggle(&self, id: i32) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE osc_toggle_states
            SET is_active = false
            WHERE id = $1 AND is_active = true
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        Ok(())
    }
    
    async fn cleanup_expired_toggles(&self) -> Result<i64, Error> {
        let result = sqlx::query(
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
        let row = sqlx::query(
            r#"
            SELECT 
                id,
                avatar_id,
                avatar_name,
                parameter_mappings,
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as created_at,
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as updated_at
            FROM osc_avatar_configs
            WHERE avatar_id = $1
            "#,
        )
        .bind(avatar_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        if let Some(r) = row {
            let config = OscAvatarConfig {
                id: r.try_get("id")?,
                avatar_id: r.try_get("avatar_id")?,
                avatar_name: r.try_get("avatar_name")?,
                parameter_mappings: r.try_get("parameter_mappings")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            };
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }
    
    async fn create_or_update_avatar_config(&self, config: OscAvatarConfig) -> Result<OscAvatarConfig, Error> {
        let row = sqlx::query(
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
                COALESCE(created_at, CURRENT_TIMESTAMP)::timestamptz as created_at,
                COALESCE(updated_at, CURRENT_TIMESTAMP)::timestamptz as updated_at
            "#,
        )
        .bind(&config.avatar_id)
        .bind(&config.avatar_name)
        .bind(&config.parameter_mappings)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Database(e))?;
        
        let result = OscAvatarConfig {
            id: row.try_get("id")?,
            avatar_id: row.try_get("avatar_id")?,
            avatar_name: row.try_get("avatar_name")?,
            parameter_mappings: row.try_get("parameter_mappings")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        
        Ok(result)
    }
}