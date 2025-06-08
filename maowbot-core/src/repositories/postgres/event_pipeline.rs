use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use crate::Error;

use maowbot_common::models::event_pipeline::{
    EventPipeline, PipelineFilter, PipelineAction, PipelineExecutionLog,
    PipelineExecutionStatus, PipelineSharedData, EventTypeRegistry, EventHandlerRegistry,
    CreatePipelineRequest, UpdatePipelineRequest, CreateFilterRequest, CreateActionRequest,
    HandlerType, ActionExecutionResult,
};
use maowbot_common::traits::event_pipeline_traits::{
    EventPipelineRepository, PipelineExecutionLogRepository, PipelineSharedDataRepository,
    EventTypeRegistryRepository, EventHandlerRegistryRepository, EventPipelineSystemRepository,
};

pub struct PostgresEventPipelineRepository {
    pool: Pool<Postgres>,
}

impl PostgresEventPipelineRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventPipelineRepository for PostgresEventPipelineRepository {
    async fn create_pipeline(&self, request: &CreatePipelineRequest) -> Result<EventPipeline, Error> {
        let pipeline_id = Uuid::new_v4();
        let metadata = request.metadata.clone().unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        let now = Utc::now();
        
        let row = sqlx::query(
            r#"
            INSERT INTO event_pipelines 
                (pipeline_id, name, description, enabled, priority, stop_on_match, stop_on_error, 
                 tags, metadata, created_at, updated_at, execution_count, success_count, is_system)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING *
            "#,
        )
        .bind(pipeline_id)
        .bind(&request.name)
        .bind(&request.description)
        .bind(request.enabled)
        .bind(request.priority)
        .bind(request.stop_on_match)
        .bind(request.stop_on_error)
        .bind(&request.tags)
        .bind(metadata)
        .bind(now)
        .bind(now)
        .bind(0i64) // execution_count
        .bind(0i64) // success_count
        .bind(false) // is_system
        .fetch_one(&self.pool)
        .await?;
        
        Ok(EventPipeline {
            pipeline_id: row.try_get("pipeline_id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            enabled: row.try_get("enabled")?,
            priority: row.try_get("priority")?,
            stop_on_match: row.try_get("stop_on_match")?,
            stop_on_error: row.try_get("stop_on_error")?,
            created_by: row.try_get("created_by")?,
            is_system: row.try_get("is_system")?,
            tags: row.try_get("tags")?,
            metadata: row.try_get("metadata")?,
            execution_count: row.try_get("execution_count")?,
            success_count: row.try_get("success_count")?,
            last_executed: row.try_get("last_executed")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
    
    async fn get_pipeline(&self, pipeline_id: Uuid) -> Result<Option<EventPipeline>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM event_pipelines WHERE pipeline_id = $1"
        )
        .bind(pipeline_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(r) = row_opt {
            Ok(Some(EventPipeline {
                pipeline_id: r.try_get("pipeline_id")?,
                name: r.try_get("name")?,
                description: r.try_get("description")?,
                enabled: r.try_get("enabled")?,
                priority: r.try_get("priority")?,
                stop_on_match: r.try_get("stop_on_match")?,
                stop_on_error: r.try_get("stop_on_error")?,
                created_by: r.try_get("created_by")?,
                is_system: r.try_get("is_system")?,
                tags: r.try_get("tags")?,
                metadata: r.try_get("metadata")?,
                execution_count: r.try_get("execution_count")?,
                success_count: r.try_get("success_count")?,
                last_executed: r.try_get("last_executed")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn get_pipeline_by_name(&self, name: &str) -> Result<Option<EventPipeline>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM event_pipelines WHERE name = $1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(r) = row_opt {
            Ok(Some(EventPipeline {
                pipeline_id: r.try_get("pipeline_id")?,
                name: r.try_get("name")?,
                description: r.try_get("description")?,
                enabled: r.try_get("enabled")?,
                priority: r.try_get("priority")?,
                stop_on_match: r.try_get("stop_on_match")?,
                stop_on_error: r.try_get("stop_on_error")?,
                created_by: r.try_get("created_by")?,
                is_system: r.try_get("is_system")?,
                tags: r.try_get("tags")?,
                metadata: r.try_get("metadata")?,
                execution_count: r.try_get("execution_count")?,
                success_count: r.try_get("success_count")?,
                last_executed: r.try_get("last_executed")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn list_pipelines(&self, enabled_only: bool) -> Result<Vec<EventPipeline>, Error> {
        let query = if enabled_only {
            "SELECT * FROM event_pipelines WHERE enabled = true ORDER BY priority, name"
        } else {
            "SELECT * FROM event_pipelines ORDER BY priority, name"
        };
        
        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await?;
        
        let mut pipelines = Vec::new();
        for r in rows {
            pipelines.push(EventPipeline {
                pipeline_id: r.try_get("pipeline_id")?,
                name: r.try_get("name")?,
                description: r.try_get("description")?,
                enabled: r.try_get("enabled")?,
                priority: r.try_get("priority")?,
                stop_on_match: r.try_get("stop_on_match")?,
                stop_on_error: r.try_get("stop_on_error")?,
                created_by: r.try_get("created_by")?,
                is_system: r.try_get("is_system")?,
                tags: r.try_get("tags")?,
                metadata: r.try_get("metadata")?,
                execution_count: r.try_get("execution_count")?,
                success_count: r.try_get("success_count")?,
                last_executed: r.try_get("last_executed")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            });
        }
        Ok(pipelines)
    }
    
    async fn list_pipelines_by_tag(&self, tag: &str) -> Result<Vec<EventPipeline>, Error> {
        let rows = sqlx::query(
            "SELECT * FROM event_pipelines WHERE $1 = ANY(tags) ORDER BY priority, name"
        )
        .bind(tag)
        .fetch_all(&self.pool)
        .await?;
        
        let mut pipelines = Vec::new();
        for r in rows {
            pipelines.push(EventPipeline {
                pipeline_id: r.try_get("pipeline_id")?,
                name: r.try_get("name")?,
                description: r.try_get("description")?,
                enabled: r.try_get("enabled")?,
                priority: r.try_get("priority")?,
                stop_on_match: r.try_get("stop_on_match")?,
                stop_on_error: r.try_get("stop_on_error")?,
                created_by: r.try_get("created_by")?,
                is_system: r.try_get("is_system")?,
                tags: r.try_get("tags")?,
                metadata: r.try_get("metadata")?,
                execution_count: r.try_get("execution_count")?,
                success_count: r.try_get("success_count")?,
                last_executed: r.try_get("last_executed")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            });
        }
        Ok(pipelines)
    }
    
    async fn update_pipeline(&self, pipeline_id: Uuid, request: &UpdatePipelineRequest) -> Result<EventPipeline, Error> {
        let row = sqlx::query(
            r#"
            UPDATE event_pipelines 
            SET 
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                enabled = COALESCE($4, enabled),
                priority = COALESCE($5, priority),
                stop_on_match = COALESCE($6, stop_on_match),
                stop_on_error = COALESCE($7, stop_on_error),
                tags = COALESCE($8, tags),
                metadata = COALESCE($9, metadata),
                updated_at = NOW()
            WHERE pipeline_id = $1
            RETURNING *
            "#
        )
        .bind(pipeline_id)
        .bind(&request.name)
        .bind(&request.description)
        .bind(request.enabled)
        .bind(request.priority)
        .bind(request.stop_on_match)
        .bind(request.stop_on_error)
        .bind(request.tags.as_deref())
        .bind(&request.metadata)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(EventPipeline {
            pipeline_id: row.try_get("pipeline_id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            enabled: row.try_get("enabled")?,
            priority: row.try_get("priority")?,
            stop_on_match: row.try_get("stop_on_match")?,
            stop_on_error: row.try_get("stop_on_error")?,
            created_by: row.try_get("created_by")?,
            is_system: row.try_get("is_system")?,
            tags: row.try_get("tags")?,
            metadata: row.try_get("metadata")?,
            execution_count: row.try_get("execution_count")?,
            success_count: row.try_get("success_count")?,
            last_executed: row.try_get("last_executed")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
    
    async fn delete_pipeline(&self, pipeline_id: Uuid) -> Result<(), Error> {
        let result = sqlx::query(
            "DELETE FROM event_pipelines WHERE pipeline_id = $1 AND is_system = false"
        )
        .bind(pipeline_id)
        .execute(&self.pool)
        .await?;
        
        if result.rows_affected() == 0 {
            return Err(Error::NotFound("Pipeline not found or is a system pipeline".to_string()));
        }
        
        Ok(())
    }
    
    async fn add_filter(&self, pipeline_id: Uuid, request: &CreateFilterRequest) -> Result<PipelineFilter, Error> {
        let filter_id = Uuid::new_v4();
        let now = Utc::now();
        
        let row = sqlx::query(
            r#"
            INSERT INTO pipeline_filters 
                (filter_id, pipeline_id, filter_type, filter_config, filter_order, is_negated, is_required, created_at, updated_at)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#
        )
        .bind(filter_id)
        .bind(pipeline_id)
        .bind(&request.filter_type)
        .bind(&request.filter_config)
        .bind(request.filter_order)
        .bind(request.is_negated)
        .bind(request.is_required)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(PipelineFilter {
            filter_id: row.try_get("filter_id")?,
            pipeline_id: row.try_get("pipeline_id")?,
            filter_type: row.try_get("filter_type")?,
            filter_config: row.try_get("filter_config")?,
            filter_order: row.try_get("filter_order")?,
            is_negated: row.try_get("is_negated")?,
            is_required: row.try_get("is_required")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
    
    async fn get_filter(&self, filter_id: Uuid) -> Result<Option<PipelineFilter>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM pipeline_filters WHERE filter_id = $1"
        )
        .bind(filter_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(PipelineFilter {
                filter_id: row.try_get("filter_id")?,
                pipeline_id: row.try_get("pipeline_id")?,
                filter_type: row.try_get("filter_type")?,
                filter_config: row.try_get("filter_config")?,
                filter_order: row.try_get("filter_order")?,
                is_negated: row.try_get("is_negated")?,
                is_required: row.try_get("is_required")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn list_filters_for_pipeline(&self, pipeline_id: Uuid) -> Result<Vec<PipelineFilter>, Error> {
        let rows = sqlx::query(
            "SELECT * FROM pipeline_filters WHERE pipeline_id = $1 ORDER BY filter_order"
        )
        .bind(pipeline_id)
        .fetch_all(&self.pool)
        .await?;
        
        let mut filters = Vec::new();
        for row in rows {
            filters.push(PipelineFilter {
                filter_id: row.try_get("filter_id")?,
                pipeline_id: row.try_get("pipeline_id")?,
                filter_type: row.try_get("filter_type")?,
                filter_config: row.try_get("filter_config")?,
                filter_order: row.try_get("filter_order")?,
                is_negated: row.try_get("is_negated")?,
                is_required: row.try_get("is_required")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(filters)
    }
    
    async fn update_filter(&self, filter_id: Uuid, request: &CreateFilterRequest) -> Result<PipelineFilter, Error> {
        let row = sqlx::query(
            r#"
            UPDATE pipeline_filters 
            SET 
                filter_type = $2,
                filter_config = $3,
                filter_order = $4,
                is_negated = $5,
                is_required = $6,
                updated_at = NOW()
            WHERE filter_id = $1
            RETURNING *
            "#
        )
        .bind(filter_id)
        .bind(&request.filter_type)
        .bind(&request.filter_config)
        .bind(request.filter_order)
        .bind(request.is_negated)
        .bind(request.is_required)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(PipelineFilter {
            filter_id: row.try_get("filter_id")?,
            pipeline_id: row.try_get("pipeline_id")?,
            filter_type: row.try_get("filter_type")?,
            filter_config: row.try_get("filter_config")?,
            filter_order: row.try_get("filter_order")?,
            is_negated: row.try_get("is_negated")?,
            is_required: row.try_get("is_required")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
    
    async fn delete_filter(&self, filter_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "DELETE FROM pipeline_filters WHERE filter_id = $1"
        )
        .bind(filter_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn reorder_filters(&self, pipeline_id: Uuid, filter_ids: Vec<Uuid>) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        
        for (index, filter_id) in filter_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE pipeline_filters SET filter_order = $1 WHERE filter_id = $2 AND pipeline_id = $3"
            )
            .bind(index as i32)
            .bind(filter_id)
            .bind(pipeline_id)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }
    
    async fn add_action(&self, pipeline_id: Uuid, request: &CreateActionRequest) -> Result<PipelineAction, Error> {
        let action_id = Uuid::new_v4();
        let now = Utc::now();
        
        let row = sqlx::query(
            r#"
            INSERT INTO pipeline_actions 
                (action_id, pipeline_id, action_type, action_config, action_order, 
                 continue_on_error, is_async, timeout_ms, retry_count, retry_delay_ms,
                 condition_type, condition_config, created_at, updated_at)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING *
            "#
        )
        .bind(action_id)
        .bind(pipeline_id)
        .bind(&request.action_type)
        .bind(&request.action_config)
        .bind(request.action_order)
        .bind(request.continue_on_error)
        .bind(request.is_async)
        .bind(request.timeout_ms)
        .bind(request.retry_count)
        .bind(request.retry_delay_ms)
        .bind(&request.condition_type)
        .bind(&request.condition_config)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(PipelineAction {
            action_id: row.try_get("action_id")?,
            pipeline_id: row.try_get("pipeline_id")?,
            action_type: row.try_get("action_type")?,
            action_config: row.try_get("action_config")?,
            action_order: row.try_get("action_order")?,
            continue_on_error: row.try_get("continue_on_error")?,
            is_async: row.try_get("is_async")?,
            timeout_ms: row.try_get("timeout_ms")?,
            retry_count: row.try_get("retry_count")?,
            retry_delay_ms: row.try_get("retry_delay_ms")?,
            condition_type: row.try_get("condition_type")?,
            condition_config: row.try_get("condition_config")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
    
    async fn get_action(&self, action_id: Uuid) -> Result<Option<PipelineAction>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM pipeline_actions WHERE action_id = $1"
        )
        .bind(action_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(PipelineAction {
                action_id: row.try_get("action_id")?,
                pipeline_id: row.try_get("pipeline_id")?,
                action_type: row.try_get("action_type")?,
                action_config: row.try_get("action_config")?,
                action_order: row.try_get("action_order")?,
                continue_on_error: row.try_get("continue_on_error")?,
                is_async: row.try_get("is_async")?,
                timeout_ms: row.try_get("timeout_ms")?,
                retry_count: row.try_get("retry_count")?,
                retry_delay_ms: row.try_get("retry_delay_ms")?,
                condition_type: row.try_get("condition_type")?,
                condition_config: row.try_get("condition_config")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn list_actions_for_pipeline(&self, pipeline_id: Uuid) -> Result<Vec<PipelineAction>, Error> {
        let rows = sqlx::query(
            "SELECT * FROM pipeline_actions WHERE pipeline_id = $1 ORDER BY action_order"
        )
        .bind(pipeline_id)
        .fetch_all(&self.pool)
        .await?;
        
        let mut actions = Vec::new();
        for row in rows {
            actions.push(PipelineAction {
                action_id: row.try_get("action_id")?,
                pipeline_id: row.try_get("pipeline_id")?,
                action_type: row.try_get("action_type")?,
                action_config: row.try_get("action_config")?,
                action_order: row.try_get("action_order")?,
                continue_on_error: row.try_get("continue_on_error")?,
                is_async: row.try_get("is_async")?,
                timeout_ms: row.try_get("timeout_ms")?,
                retry_count: row.try_get("retry_count")?,
                retry_delay_ms: row.try_get("retry_delay_ms")?,
                condition_type: row.try_get("condition_type")?,
                condition_config: row.try_get("condition_config")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(actions)
    }
    
    async fn update_action(&self, action_id: Uuid, request: &CreateActionRequest) -> Result<PipelineAction, Error> {
        let row = sqlx::query(
            r#"
            UPDATE pipeline_actions 
            SET 
                action_type = $2,
                action_config = $3,
                action_order = $4,
                continue_on_error = $5,
                is_async = $6,
                timeout_ms = $7,
                retry_count = $8,
                retry_delay_ms = $9,
                condition_type = $10,
                condition_config = $11,
                updated_at = NOW()
            WHERE action_id = $1
            RETURNING *
            "#
        )
        .bind(action_id)
        .bind(&request.action_type)
        .bind(&request.action_config)
        .bind(request.action_order)
        .bind(request.continue_on_error)
        .bind(request.is_async)
        .bind(request.timeout_ms)
        .bind(request.retry_count)
        .bind(request.retry_delay_ms)
        .bind(&request.condition_type)
        .bind(&request.condition_config)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(PipelineAction {
            action_id: row.try_get("action_id")?,
            pipeline_id: row.try_get("pipeline_id")?,
            action_type: row.try_get("action_type")?,
            action_config: row.try_get("action_config")?,
            action_order: row.try_get("action_order")?,
            continue_on_error: row.try_get("continue_on_error")?,
            is_async: row.try_get("is_async")?,
            timeout_ms: row.try_get("timeout_ms")?,
            retry_count: row.try_get("retry_count")?,
            retry_delay_ms: row.try_get("retry_delay_ms")?,
            condition_type: row.try_get("condition_type")?,
            condition_config: row.try_get("condition_config")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
    
    async fn delete_action(&self, action_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "DELETE FROM pipeline_actions WHERE action_id = $1"
        )
        .bind(action_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn reorder_actions(&self, pipeline_id: Uuid, action_ids: Vec<Uuid>) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        
        for (index, action_id) in action_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE pipeline_actions SET action_order = $1 WHERE action_id = $2 AND pipeline_id = $3"
            )
            .bind(index as i32)
            .bind(action_id)
            .bind(pipeline_id)
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }
    
    async fn get_pipelines_for_event(&self, event_type: &str, platform: &str) -> Result<Vec<EventPipeline>, Error> {
        // This would need to check filters to see which pipelines match
        // For now, return all enabled pipelines ordered by priority
        let rows = sqlx::query(
            r#"
            SELECT p.* FROM event_pipelines p
            WHERE p.enabled = true
            ORDER BY p.priority, p.name
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut pipelines = Vec::new();
        for r in rows {
            pipelines.push(EventPipeline {
                pipeline_id: r.try_get("pipeline_id")?,
                name: r.try_get("name")?,
                description: r.try_get("description")?,
                enabled: r.try_get("enabled")?,
                priority: r.try_get("priority")?,
                stop_on_match: r.try_get("stop_on_match")?,
                stop_on_error: r.try_get("stop_on_error")?,
                created_by: r.try_get("created_by")?,
                is_system: r.try_get("is_system")?,
                tags: r.try_get("tags")?,
                metadata: r.try_get("metadata")?,
                execution_count: r.try_get("execution_count")?,
                success_count: r.try_get("success_count")?,
                last_executed: r.try_get("last_executed")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            });
        }
        Ok(pipelines)
    }
    
    async fn increment_execution_stats(&self, pipeline_id: Uuid, success: bool) -> Result<(), Error> {
        if success {
            sqlx::query(
                r#"
                UPDATE event_pipelines 
                SET 
                    execution_count = execution_count + 1,
                    success_count = success_count + 1,
                    last_executed = NOW()
                WHERE pipeline_id = $1
                "#
            )
            .bind(pipeline_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE event_pipelines 
                SET 
                    execution_count = execution_count + 1,
                    last_executed = NOW()
                WHERE pipeline_id = $1
                "#
            )
            .bind(pipeline_id)
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl PipelineExecutionLogRepository for PostgresEventPipelineRepository {
    async fn create_execution(&self, pipeline_id: Uuid, event_type: &str, event_data: serde_json::Value) -> Result<PipelineExecutionLog, Error> {
        let execution_id = Uuid::new_v4();
        let now = Utc::now();
        
        let row = sqlx::query(
            r#"
            INSERT INTO pipeline_execution_log 
                (execution_id, pipeline_id, event_type, event_data, started_at, status, actions_executed, actions_succeeded, action_results)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#
        )
        .bind(execution_id)
        .bind(pipeline_id)
        .bind(event_type)
        .bind(&event_data)
        .bind(now)
        .bind("running")
        .bind(0)
        .bind(0)
        .bind(serde_json::json!([]))
        .fetch_one(&self.pool)
        .await?;
        
        Ok(PipelineExecutionLog {
            execution_id: row.try_get("execution_id")?,
            pipeline_id: row.try_get("pipeline_id")?,
            event_type: row.try_get("event_type")?,
            event_data: row.try_get("event_data")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
            duration_ms: row.try_get("duration_ms")?,
            status: match row.try_get::<String, _>("status")?.as_str() {
                "running" => PipelineExecutionStatus::Running,
                "success" => PipelineExecutionStatus::Success,
                "failed" => PipelineExecutionStatus::Failed,
                "timeout" => PipelineExecutionStatus::Timeout,
                "cancelled" => PipelineExecutionStatus::Cancelled,
                _ => PipelineExecutionStatus::Failed,
            },
            error_message: row.try_get("error_message")?,
            actions_executed: row.try_get::<Option<i32>, _>("actions_executed")?.unwrap_or(0),
            actions_succeeded: row.try_get::<Option<i32>, _>("actions_succeeded")?.unwrap_or(0),
            action_results: serde_json::from_value(row.try_get::<Option<serde_json::Value>, _>("action_results")?.unwrap_or(serde_json::json!([])))
                .unwrap_or_default(),
            triggered_by: row.try_get("triggered_by")?,
            platform: row.try_get("platform")?,
        })
    }
    
    async fn get_execution(&self, execution_id: Uuid) -> Result<Option<PipelineExecutionLog>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM pipeline_execution_log WHERE execution_id = $1"
        )
        .bind(execution_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(PipelineExecutionLog {
                execution_id: row.try_get("execution_id")?,
                pipeline_id: row.try_get("pipeline_id")?,
                event_type: row.try_get("event_type")?,
                event_data: row.try_get("event_data")?,
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
                duration_ms: row.try_get("duration_ms")?,
                status: match row.try_get::<String, _>("status")?.as_str() {
                    "running" => PipelineExecutionStatus::Running,
                    "success" => PipelineExecutionStatus::Success,
                    "failed" => PipelineExecutionStatus::Failed,
                    "timeout" => PipelineExecutionStatus::Timeout,
                    "cancelled" => PipelineExecutionStatus::Cancelled,
                    _ => PipelineExecutionStatus::Failed,
                },
                error_message: row.try_get("error_message")?,
                actions_executed: row.try_get::<Option<i32>, _>("actions_executed")?.unwrap_or(0),
                actions_succeeded: row.try_get::<Option<i32>, _>("actions_succeeded")?.unwrap_or(0),
                action_results: serde_json::from_value(row.try_get::<Option<serde_json::Value>, _>("action_results")?.unwrap_or(serde_json::json!([])))
                    .unwrap_or_default(),
                triggered_by: row.try_get("triggered_by")?,
                platform: row.try_get("platform")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn update_execution_status(
        &self, 
        execution_id: Uuid, 
        status: PipelineExecutionStatus, 
        error_message: Option<String>
    ) -> Result<(), Error> {
        let status_str = match status {
            PipelineExecutionStatus::Running => "running",
            PipelineExecutionStatus::Success => "success",
            PipelineExecutionStatus::Failed => "failed",
            PipelineExecutionStatus::Timeout => "timeout",
            PipelineExecutionStatus::Cancelled => "cancelled",
        };
        
        sqlx::query(
            r#"
            UPDATE pipeline_execution_log 
            SET 
                status = $2,
                error_message = $3,
                completed_at = CASE WHEN $2 != 'running' THEN NOW() ELSE completed_at END,
                duration_ms = CASE WHEN $2 != 'running' THEN EXTRACT(EPOCH FROM (NOW() - started_at)) * 1000 ELSE duration_ms END
            WHERE execution_id = $1
            "#
        )
        .bind(execution_id)
        .bind(status_str)
        .bind(error_message)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn add_action_result(
        &self,
        execution_id: Uuid,
        action_result: serde_json::Value
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE pipeline_execution_log 
            SET 
                action_results = action_results || $2::jsonb,
                actions_executed = actions_executed + 1,
                actions_succeeded = CASE 
                    WHEN ($2->>'status')::text = 'success' 
                    THEN actions_succeeded + 1 
                    ELSE actions_succeeded 
                END
            WHERE execution_id = $1
            "#
        )
        .bind(execution_id)
        .bind(serde_json::json!([action_result]))
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn list_executions_for_pipeline(
        &self, 
        pipeline_id: Uuid, 
        limit: i64,
        status_filter: Option<PipelineExecutionStatus>
    ) -> Result<Vec<PipelineExecutionLog>, Error> {
        let rows = if let Some(status) = status_filter {
            let status_str = match status {
                PipelineExecutionStatus::Running => "running",
                PipelineExecutionStatus::Success => "success",
                PipelineExecutionStatus::Failed => "failed",
                PipelineExecutionStatus::Timeout => "timeout",
                PipelineExecutionStatus::Cancelled => "cancelled",
            };
            
            sqlx::query(
                r#"
                SELECT * FROM pipeline_execution_log 
                WHERE pipeline_id = $1 AND status = $2
                ORDER BY started_at DESC
                LIMIT $3
                "#
            )
            .bind(pipeline_id)
            .bind(status_str)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT * FROM pipeline_execution_log 
                WHERE pipeline_id = $1
                ORDER BY started_at DESC
                LIMIT $2
                "#
            )
            .bind(pipeline_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        
        let mut executions = Vec::new();
        for row in rows {
            executions.push(PipelineExecutionLog {
                execution_id: row.try_get("execution_id")?,
                pipeline_id: row.try_get("pipeline_id")?,
                event_type: row.try_get("event_type")?,
                event_data: row.try_get("event_data")?,
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
                duration_ms: row.try_get("duration_ms")?,
                status: match row.try_get::<String, _>("status")?.as_str() {
                    "running" => PipelineExecutionStatus::Running,
                    "success" => PipelineExecutionStatus::Success,
                    "failed" => PipelineExecutionStatus::Failed,
                    "timeout" => PipelineExecutionStatus::Timeout,
                    "cancelled" => PipelineExecutionStatus::Cancelled,
                    _ => PipelineExecutionStatus::Failed,
                },
                error_message: row.try_get("error_message")?,
                actions_executed: row.try_get::<Option<i32>, _>("actions_executed")?.unwrap_or(0),
                actions_succeeded: row.try_get::<Option<i32>, _>("actions_succeeded")?.unwrap_or(0),
                action_results: serde_json::from_value(row.try_get::<Option<serde_json::Value>, _>("action_results")?.unwrap_or(serde_json::json!([])))
                    .unwrap_or_default(),
                triggered_by: row.try_get("triggered_by")?,
                platform: row.try_get("platform")?,
            });
        }
        Ok(executions)
    }
    
    async fn list_recent_executions(&self, limit: i64) -> Result<Vec<PipelineExecutionLog>, Error> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM pipeline_execution_log 
            ORDER BY started_at DESC
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        let mut executions = Vec::new();
        for row in rows {
            executions.push(PipelineExecutionLog {
                execution_id: row.try_get("execution_id")?,
                pipeline_id: row.try_get("pipeline_id")?,
                event_type: row.try_get("event_type")?,
                event_data: row.try_get("event_data")?,
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
                duration_ms: row.try_get("duration_ms")?,
                status: match row.try_get::<String, _>("status")?.as_str() {
                    "running" => PipelineExecutionStatus::Running,
                    "success" => PipelineExecutionStatus::Success,
                    "failed" => PipelineExecutionStatus::Failed,
                    "timeout" => PipelineExecutionStatus::Timeout,
                    "cancelled" => PipelineExecutionStatus::Cancelled,
                    _ => PipelineExecutionStatus::Failed,
                },
                error_message: row.try_get("error_message")?,
                actions_executed: row.try_get::<Option<i32>, _>("actions_executed")?.unwrap_or(0),
                actions_succeeded: row.try_get::<Option<i32>, _>("actions_succeeded")?.unwrap_or(0),
                action_results: serde_json::from_value(row.try_get::<Option<serde_json::Value>, _>("action_results")?.unwrap_or(serde_json::json!([])))
                    .unwrap_or_default(),
                triggered_by: row.try_get("triggered_by")?,
                platform: row.try_get("platform")?,
            });
        }
        Ok(executions)
    }
    
    async fn cleanup_old_executions(&self, older_than: DateTime<Utc>) -> Result<i64, Error> {
        let result = sqlx::query(
            "DELETE FROM pipeline_execution_log WHERE started_at < $1"
        )
        .bind(older_than)
        .execute(&self.pool)
        .await?;
        
        Ok(result.rows_affected() as i64)
    }
}

#[async_trait]
impl PipelineSharedDataRepository for PostgresEventPipelineRepository {
    async fn set_shared_data(
        &self,
        execution_id: Uuid,
        key: &str,
        value: serde_json::Value,
        data_type: Option<String>,
        set_by_action: Option<Uuid>
    ) -> Result<(), Error> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT INTO pipeline_shared_data 
                (shared_data_id, execution_id, data_key, data_value, data_type, set_by_action, created_at)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (execution_id, data_key) 
            DO UPDATE SET 
                data_value = EXCLUDED.data_value,
                data_type = EXCLUDED.data_type,
                set_by_action = EXCLUDED.set_by_action
            "#
        )
        .bind(Uuid::new_v4())
        .bind(execution_id)
        .bind(key)
        .bind(&value)
        .bind(data_type)
        .bind(set_by_action)
        .bind(now)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_shared_data(&self, execution_id: Uuid, key: &str) -> Result<Option<PipelineSharedData>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM pipeline_shared_data WHERE execution_id = $1 AND data_key = $2"
        )
        .bind(execution_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(PipelineSharedData {
                shared_data_id: row.try_get("shared_data_id")?,
                execution_id: row.try_get("execution_id")?,
                data_key: row.try_get("data_key")?,
                data_value: row.try_get("data_value")?,
                data_type: row.try_get("data_type")?,
                set_by_action: row.try_get("set_by_action")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn list_shared_data(&self, execution_id: Uuid) -> Result<Vec<PipelineSharedData>, Error> {
        let rows = sqlx::query(
            "SELECT * FROM pipeline_shared_data WHERE execution_id = $1 ORDER BY created_at"
        )
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await?;
        
        let mut data_list = Vec::new();
        for row in rows {
            data_list.push(PipelineSharedData {
                shared_data_id: row.try_get("shared_data_id")?,
                execution_id: row.try_get("execution_id")?,
                data_key: row.try_get("data_key")?,
                data_value: row.try_get("data_value")?,
                data_type: row.try_get("data_type")?,
                set_by_action: row.try_get("set_by_action")?,
                created_at: row.try_get("created_at")?,
            });
        }
        Ok(data_list)
    }
    
    async fn delete_shared_data(&self, execution_id: Uuid, key: &str) -> Result<(), Error> {
        sqlx::query(
            "DELETE FROM pipeline_shared_data WHERE execution_id = $1 AND data_key = $2"
        )
        .bind(execution_id)
        .bind(key)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

#[async_trait]
impl EventTypeRegistryRepository for PostgresEventPipelineRepository {
    async fn register_event_type(
        &self,
        platform: &str,
        category: &str,
        name: &str,
        description: Option<String>,
        schema: Option<serde_json::Value>
    ) -> Result<EventTypeRegistry, Error> {
        let event_type_id = Uuid::new_v4();
        let now = Utc::now();
        
        let row = sqlx::query(
            r#"
            INSERT INTO event_type_registry 
                (event_type_id, platform, event_category, event_name, description, event_schema, is_enabled, created_at)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (platform, event_name) 
            DO UPDATE SET 
                event_category = EXCLUDED.event_category,
                description = EXCLUDED.description,
                event_schema = EXCLUDED.event_schema
            RETURNING *
            "#
        )
        .bind(event_type_id)
        .bind(platform)
        .bind(category)
        .bind(name)
        .bind(description)
        .bind(schema)
        .bind(true)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(EventTypeRegistry {
            event_type_id: row.try_get("event_type_id")?,
            platform: row.try_get("platform")?,
            event_category: row.try_get("event_category")?,
            event_name: row.try_get("event_name")?,
            description: row.try_get("description")?,
            event_schema: row.try_get("event_schema")?,
            is_enabled: row.try_get("is_enabled")?,
            created_at: row.try_get("created_at")?,
        })
    }
    
    async fn get_event_type(&self, event_type_id: Uuid) -> Result<Option<EventTypeRegistry>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM event_type_registry WHERE event_type_id = $1"
        )
        .bind(event_type_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(EventTypeRegistry {
                event_type_id: row.try_get("event_type_id")?,
                platform: row.try_get("platform")?,
                event_category: row.try_get("event_category")?,
                event_name: row.try_get("event_name")?,
                description: row.try_get("description")?,
                event_schema: row.try_get("event_schema")?,
                is_enabled: row.try_get("is_enabled")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn get_event_type_by_name(&self, platform: &str, event_name: &str) -> Result<Option<EventTypeRegistry>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM event_type_registry WHERE platform = $1 AND event_name = $2"
        )
        .bind(platform)
        .bind(event_name)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(EventTypeRegistry {
                event_type_id: row.try_get("event_type_id")?,
                platform: row.try_get("platform")?,
                event_category: row.try_get("event_category")?,
                event_name: row.try_get("event_name")?,
                description: row.try_get("description")?,
                event_schema: row.try_get("event_schema")?,
                is_enabled: row.try_get("is_enabled")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn list_event_types(&self, platform: Option<&str>) -> Result<Vec<EventTypeRegistry>, Error> {
        let rows = if let Some(platform) = platform {
            sqlx::query(
                "SELECT * FROM event_type_registry WHERE platform = $1 ORDER BY event_category, event_name"
            )
            .bind(platform)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT * FROM event_type_registry ORDER BY platform, event_category, event_name"
            )
            .fetch_all(&self.pool)
            .await?
        };
        
        let mut event_types = Vec::new();
        for row in rows {
            event_types.push(EventTypeRegistry {
                event_type_id: row.try_get("event_type_id")?,
                platform: row.try_get("platform")?,
                event_category: row.try_get("event_category")?,
                event_name: row.try_get("event_name")?,
                description: row.try_get("description")?,
                event_schema: row.try_get("event_schema")?,
                is_enabled: row.try_get("is_enabled")?,
                created_at: row.try_get("created_at")?,
            });
        }
        Ok(event_types)
    }
    
    async fn update_event_type(&self, event_type_id: Uuid, enabled: bool) -> Result<(), Error> {
        sqlx::query(
            "UPDATE event_type_registry SET is_enabled = $2 WHERE event_type_id = $1"
        )
        .bind(event_type_id)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn delete_event_type(&self, event_type_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "DELETE FROM event_type_registry WHERE event_type_id = $1"
        )
        .bind(event_type_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

#[async_trait]
impl EventHandlerRegistryRepository for PostgresEventPipelineRepository {
    async fn register_handler(
        &self,
        handler_type: HandlerType,
        name: &str,
        category: &str,
        description: Option<String>,
        parameters: Option<serde_json::Value>,
        plugin_id: Option<String>
    ) -> Result<EventHandlerRegistry, Error> {
        let handler_id = Uuid::new_v4();
        let handler_type_str = match handler_type {
            HandlerType::Filter => "filter",
            HandlerType::Action => "action",
        };
        let now = Utc::now();
        
        let row = sqlx::query(
            r#"
            INSERT INTO event_handler_registry 
                (handler_id, handler_type, handler_name, handler_category, description, parameters, plugin_id, is_builtin, is_enabled, created_at, updated_at)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#
        )
        .bind(handler_id)
        .bind(handler_type_str)
        .bind(name)
        .bind(category)
        .bind(description)
        .bind(parameters)
        .bind(&plugin_id)
        .bind(plugin_id.is_none()) // builtin if no plugin_id
        .bind(true)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(EventHandlerRegistry {
            handler_id: row.try_get("handler_id")?,
            handler_type: match row.try_get::<String, _>("handler_type")?.as_str() {
                "filter" => HandlerType::Filter,
                "action" => HandlerType::Action,
                _ => HandlerType::Action,
            },
            handler_name: row.try_get("handler_name")?,
            handler_category: row.try_get("handler_category")?,
            description: row.try_get("description")?,
            parameters: row.try_get("parameters")?,
            is_builtin: row.try_get("is_builtin")?,
            plugin_id: row.try_get("plugin_id")?,
            is_enabled: row.try_get("is_enabled")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
    
    async fn get_handler(&self, handler_id: Uuid) -> Result<Option<EventHandlerRegistry>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM event_handler_registry WHERE handler_id = $1"
        )
        .bind(handler_id)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(EventHandlerRegistry {
                handler_id: row.try_get("handler_id")?,
                handler_type: match row.try_get::<String, _>("handler_type")?.as_str() {
                    "filter" => HandlerType::Filter,
                    "action" => HandlerType::Action,
                    _ => HandlerType::Action,
                },
                handler_name: row.try_get("handler_name")?,
                handler_category: row.try_get("handler_category")?,
                description: row.try_get("description")?,
                parameters: row.try_get("parameters")?,
                is_builtin: row.try_get("is_builtin")?,
                plugin_id: row.try_get("plugin_id")?,
                is_enabled: row.try_get("is_enabled")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn get_handler_by_name(&self, handler_name: &str) -> Result<Option<EventHandlerRegistry>, Error> {
        let row_opt = sqlx::query(
            "SELECT * FROM event_handler_registry WHERE handler_name = $1"
        )
        .bind(handler_name)
        .fetch_optional(&self.pool)
        .await?;
        
        if let Some(row) = row_opt {
            Ok(Some(EventHandlerRegistry {
                handler_id: row.try_get("handler_id")?,
                handler_type: match row.try_get::<String, _>("handler_type")?.as_str() {
                    "filter" => HandlerType::Filter,
                    "action" => HandlerType::Action,
                    _ => HandlerType::Action,
                },
                handler_name: row.try_get("handler_name")?,
                handler_category: row.try_get("handler_category")?,
                description: row.try_get("description")?,
                parameters: row.try_get("parameters")?,
                is_builtin: row.try_get("is_builtin")?,
                plugin_id: row.try_get("plugin_id")?,
                is_enabled: row.try_get("is_enabled")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
    
    async fn list_handlers(&self, handler_type: Option<HandlerType>) -> Result<Vec<EventHandlerRegistry>, Error> {
        let rows = if let Some(handler_type) = handler_type {
            let handler_type_str = match handler_type {
                HandlerType::Filter => "filter",
                HandlerType::Action => "action",
            };
            
            sqlx::query(
                "SELECT * FROM event_handler_registry WHERE handler_type = $1 ORDER BY handler_category, handler_name"
            )
            .bind(handler_type_str)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT * FROM event_handler_registry ORDER BY handler_type, handler_category, handler_name"
            )
            .fetch_all(&self.pool)
            .await?
        };
        
        let mut handlers = Vec::new();
        for row in rows {
            handlers.push(EventHandlerRegistry {
                handler_id: row.try_get("handler_id")?,
                handler_type: match row.try_get::<String, _>("handler_type")?.as_str() {
                    "filter" => HandlerType::Filter,
                    "action" => HandlerType::Action,
                    _ => HandlerType::Action,
                },
                handler_name: row.try_get("handler_name")?,
                handler_category: row.try_get("handler_category")?,
                description: row.try_get("description")?,
                parameters: row.try_get("parameters")?,
                is_builtin: row.try_get("is_builtin")?,
                plugin_id: row.try_get("plugin_id")?,
                is_enabled: row.try_get("is_enabled")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(handlers)
    }
    
    async fn list_handlers_by_category(&self, category: &str) -> Result<Vec<EventHandlerRegistry>, Error> {
        let rows = sqlx::query(
            "SELECT * FROM event_handler_registry WHERE handler_category = $1 ORDER BY handler_type, handler_name"
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await?;
        
        let mut handlers = Vec::new();
        for row in rows {
            handlers.push(EventHandlerRegistry {
                handler_id: row.try_get("handler_id")?,
                handler_type: match row.try_get::<String, _>("handler_type")?.as_str() {
                    "filter" => HandlerType::Filter,
                    "action" => HandlerType::Action,
                    _ => HandlerType::Action,
                },
                handler_name: row.try_get("handler_name")?,
                handler_category: row.try_get("handler_category")?,
                description: row.try_get("description")?,
                parameters: row.try_get("parameters")?,
                is_builtin: row.try_get("is_builtin")?,
                plugin_id: row.try_get("plugin_id")?,
                is_enabled: row.try_get("is_enabled")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }
        Ok(handlers)
    }
    
    async fn update_handler(&self, handler_id: Uuid, enabled: bool) -> Result<(), Error> {
        sqlx::query(
            "UPDATE event_handler_registry SET is_enabled = $2, updated_at = NOW() WHERE handler_id = $1"
        )
        .bind(handler_id)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn delete_handler(&self, handler_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "DELETE FROM event_handler_registry WHERE handler_id = $1 AND is_builtin = false"
        )
        .bind(handler_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

// Implement the combined trait
#[async_trait]
impl EventPipelineSystemRepository for PostgresEventPipelineRepository {}