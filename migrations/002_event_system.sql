-- 002_event_system.sql
-- Event handling system including commands, redeems, and the new pipeline system
-- Consolidated from original migrations 002, 003, and new pipeline design

-- Drop existing tables to ensure clean slate
DROP TABLE IF EXISTS 
    pipeline_execution_log,
    pipeline_shared_data,
    pipeline_actions,
    pipeline_filters,
    event_pipelines,
    event_handler_registry,
    event_type_registry,
    command_usage,
    commands,
    redeem_usage,
    redeems,
    link_requests
CASCADE;

---------------------------------------------------------------------------
-- EVENT TYPE REGISTRY
---------------------------------------------------------------------------

-- Registry of all possible event types in the system
CREATE TABLE event_type_registry (
    event_type_id   UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform        TEXT NOT NULL,
    event_category  TEXT NOT NULL, -- 'chat', 'stream', 'user', 'subscription', etc
    event_name      TEXT NOT NULL, -- 'message.create', 'stream.online', etc
    description     TEXT,
    event_schema    JSONB, -- JSON schema for event data validation
    is_enabled      BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT event_type_unique UNIQUE(platform, event_name),
    CONSTRAINT platform_event_check CHECK (platform IN ('twitch', 'twitch-irc', 'twitch-eventsub', 'discord', 'vrchat', 'obs', 'system'))
);

CREATE INDEX idx_event_type_platform ON event_type_registry(platform);
CREATE INDEX idx_event_type_category ON event_type_registry(event_category);

---------------------------------------------------------------------------
-- EVENT HANDLER REGISTRY
---------------------------------------------------------------------------

-- Registry of available event handlers (both built-in and plugin-provided)
CREATE TABLE event_handler_registry (
    handler_id      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    handler_type    TEXT NOT NULL, -- 'filter' or 'action'
    handler_name    TEXT NOT NULL UNIQUE,
    handler_category TEXT NOT NULL, -- 'platform', 'channel', 'message', 'user', 'time', etc
    description     TEXT,
    parameters      JSONB, -- Parameter definitions with types and validation
    is_builtin      BOOLEAN NOT NULL DEFAULT false,
    plugin_id       TEXT, -- If provided by a plugin
    is_enabled      BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT handler_type_check CHECK (handler_type IN ('filter', 'action'))
);

CREATE INDEX idx_handler_registry_type ON event_handler_registry(handler_type);
CREATE INDEX idx_handler_registry_category ON event_handler_registry(handler_category);

---------------------------------------------------------------------------
-- EVENT PIPELINE SYSTEM
---------------------------------------------------------------------------

-- Event pipelines - Core workflow definitions
CREATE TABLE event_pipelines (
    pipeline_id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name            TEXT NOT NULL,
    description     TEXT,
    enabled         BOOLEAN NOT NULL DEFAULT true,
    priority        INT NOT NULL DEFAULT 100, -- Lower numbers execute first
    stop_on_match   BOOLEAN NOT NULL DEFAULT false, -- Stop processing other pipelines if this matches
    stop_on_error   BOOLEAN NOT NULL DEFAULT false, -- Stop this pipeline on first error
    
    -- Metadata
    created_by      UUID REFERENCES users(user_id) ON DELETE SET NULL,
    is_system       BOOLEAN NOT NULL DEFAULT false, -- System pipelines can't be deleted
    tags            TEXT[] DEFAULT '{}',
    metadata        JSONB DEFAULT '{}',
    
    -- Statistics
    execution_count BIGINT NOT NULL DEFAULT 0,
    success_count   BIGINT NOT NULL DEFAULT 0,
    last_executed   TIMESTAMPTZ,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT pipeline_name_unique UNIQUE(name)
);

CREATE INDEX idx_pipelines_enabled ON event_pipelines(enabled);
CREATE INDEX idx_pipelines_priority ON event_pipelines(priority);
CREATE INDEX idx_pipelines_tags ON event_pipelines USING gin(tags);

-- Pipeline filters - Determine if a pipeline should execute
CREATE TABLE pipeline_filters (
    filter_id       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    pipeline_id     UUID NOT NULL REFERENCES event_pipelines(pipeline_id) ON DELETE CASCADE,
    filter_type     TEXT NOT NULL, -- References handler_name in event_handler_registry
    filter_config   JSONB NOT NULL DEFAULT '{}', -- Configuration specific to filter type
    filter_order    INT NOT NULL DEFAULT 0, -- Order of execution within pipeline
    is_negated      BOOLEAN NOT NULL DEFAULT false, -- Invert the filter result
    is_required     BOOLEAN NOT NULL DEFAULT true, -- Must pass for pipeline to execute
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT filter_type_exists FOREIGN KEY (filter_type) 
        REFERENCES event_handler_registry(handler_name)
);

CREATE INDEX idx_pipeline_filters_pipeline ON pipeline_filters(pipeline_id);
CREATE INDEX idx_pipeline_filters_order ON pipeline_filters(filter_order);

-- Pipeline actions - What happens when a pipeline executes
CREATE TABLE pipeline_actions (
    action_id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    pipeline_id         UUID NOT NULL REFERENCES event_pipelines(pipeline_id) ON DELETE CASCADE,
    action_type         TEXT NOT NULL, -- References handler_name in event_handler_registry
    action_config       JSONB NOT NULL DEFAULT '{}', -- Configuration specific to action type
    action_order        INT NOT NULL DEFAULT 0, -- Order of execution within pipeline
    continue_on_error   BOOLEAN NOT NULL DEFAULT true, -- Continue pipeline if this action fails
    is_async            BOOLEAN NOT NULL DEFAULT false, -- Execute asynchronously
    timeout_ms          INT, -- Action timeout in milliseconds
    retry_count         INT DEFAULT 0, -- Number of retries on failure
    retry_delay_ms      INT DEFAULT 1000, -- Delay between retries
    
    -- Conditional execution
    condition_type      TEXT, -- 'previous_success', 'previous_failure', 'data_check', etc
    condition_config    JSONB, -- Configuration for conditional execution
    
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT action_type_exists FOREIGN KEY (action_type) 
        REFERENCES event_handler_registry(handler_name),
    CONSTRAINT condition_type_check CHECK (
        condition_type IS NULL OR 
        condition_type IN ('previous_success', 'previous_failure', 'data_check', 'expression')
    )
);

CREATE INDEX idx_pipeline_actions_pipeline ON pipeline_actions(pipeline_id);
CREATE INDEX idx_pipeline_actions_order ON pipeline_actions(action_order);

-- Pipeline execution log - Track pipeline executions (partitioned by time)
CREATE TABLE pipeline_execution_log (
    execution_id    UUID NOT NULL DEFAULT uuid_generate_v4(),
    pipeline_id     UUID NOT NULL REFERENCES event_pipelines(pipeline_id) ON DELETE CASCADE,
    event_type      TEXT NOT NULL,
    event_data      JSONB NOT NULL,
    
    -- Execution details
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    duration_ms     INT,
    status          TEXT NOT NULL DEFAULT 'running', -- 'running', 'success', 'failed', 'timeout'
    error_message   TEXT,
    
    -- Action results
    actions_executed INT DEFAULT 0,
    actions_succeeded INT DEFAULT 0,
    action_results   JSONB DEFAULT '[]', -- Array of action execution details
    
    -- Context
    triggered_by    UUID REFERENCES users(user_id),
    platform        TEXT,
    
    PRIMARY KEY (execution_id, started_at),
    CONSTRAINT status_check CHECK (status IN ('running', 'success', 'failed', 'timeout', 'cancelled'))
) PARTITION BY RANGE (started_at);

CREATE INDEX idx_execution_log_pipeline ON pipeline_execution_log(pipeline_id);
CREATE INDEX idx_execution_log_started ON pipeline_execution_log(started_at DESC);
CREATE INDEX idx_execution_log_status ON pipeline_execution_log(status);

-- Index for recent execution logs (without partial index due to immutability requirement)
CREATE INDEX idx_execution_log_recent ON pipeline_execution_log(pipeline_id, started_at DESC);

-- Pipeline shared data - For passing data between actions
CREATE TABLE pipeline_shared_data (
    shared_data_id  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    execution_id    UUID NOT NULL, -- Cannot use FK with partitioned table
    data_key        TEXT NOT NULL,
    data_value      JSONB NOT NULL,
    data_type       TEXT, -- Optional type hint
    set_by_action   UUID REFERENCES pipeline_actions(action_id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT shared_data_unique UNIQUE(execution_id, data_key)
);

CREATE INDEX idx_shared_data_execution ON pipeline_shared_data(execution_id);

---------------------------------------------------------------------------
-- COMMANDS SYSTEM
---------------------------------------------------------------------------

-- Commands table (chat commands like !help)
CREATE TABLE commands (
    command_id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform                TEXT NOT NULL,
    command_name            TEXT NOT NULL,
    min_role                TEXT NOT NULL DEFAULT 'viewer',
    is_active               BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Cooldown settings
    cooldown_seconds        INT NOT NULL DEFAULT 0,
    cooldown_warnonce       BOOLEAN NOT NULL DEFAULT false,
    
    -- Credential settings
    respond_with_credential UUID REFERENCES platform_credentials(credential_id),
    active_credential_id    UUID REFERENCES platform_credentials(credential_id),
    
    -- Stream state requirements
    stream_online_only      BOOLEAN NOT NULL DEFAULT false,
    stream_offline_only     BOOLEAN NOT NULL DEFAULT false,
    
    -- Pipeline integration (new way - optional)
    pipeline_id             UUID REFERENCES event_pipelines(pipeline_id),
    
    -- Response configuration (when not using pipeline)
    default_response        TEXT, -- Default response template
    response_variations     TEXT[] DEFAULT '{}', -- Random response variations
    
    -- Plugin integration (legacy support)
    plugin_name             TEXT,
    sub_command             TEXT,
    
    CONSTRAINT commands_unique UNIQUE(platform, command_name),
    CONSTRAINT min_role_check CHECK (
        min_role IN ('viewer', 'subscriber', 'vip', 'moderator', 'broadcaster')
    )
);

CREATE INDEX idx_commands_active ON commands(is_active);
CREATE INDEX idx_commands_platform ON commands(platform, command_name);
CREATE INDEX idx_commands_plugin ON commands(plugin_name) WHERE plugin_name IS NOT NULL;

-- Command usage tracking (partitioned by time)
CREATE TABLE command_usage (
    usage_id        UUID NOT NULL DEFAULT uuid_generate_v4(),
    command_id      UUID NOT NULL REFERENCES commands(command_id) ON DELETE CASCADE,
    user_id         UUID REFERENCES users(user_id) ON DELETE SET NULL,
    platform        TEXT NOT NULL,
    channel         TEXT,
    input_text      TEXT,
    response_sent   BOOLEAN DEFAULT true,
    error_message   TEXT,
    executed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (usage_id, executed_at)
) PARTITION BY RANGE (executed_at);

CREATE INDEX idx_command_usage_command ON command_usage(command_id);
CREATE INDEX idx_command_usage_user ON command_usage(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX idx_command_usage_executed ON command_usage(executed_at DESC);

---------------------------------------------------------------------------
-- REDEEMS SYSTEM (Channel Points, etc)
---------------------------------------------------------------------------

-- Redeems table
CREATE TABLE redeems (
    redeem_id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    platform                TEXT NOT NULL,
    reward_id               TEXT NOT NULL, -- Platform-specific reward ID
    reward_name             TEXT NOT NULL,
    internal_name           TEXT UNIQUE, -- Internal identifier
    
    -- Credential for executing this redeem
    active_credential_id    UUID REFERENCES platform_credentials(credential_id),
    
    -- Pricing and availability
    cost                    INT NOT NULL DEFAULT 100,
    is_active               BOOLEAN NOT NULL DEFAULT true,
    is_paused               BOOLEAN NOT NULL DEFAULT false,
    active_offline          BOOLEAN NOT NULL DEFAULT false,
    
    -- Redemption limits
    max_per_stream          INT,
    max_per_user_per_stream INT,
    global_cooldown_secs    INT DEFAULT 0,
    
    -- Requirements
    is_input_required       BOOLEAN NOT NULL DEFAULT false,
    min_input_length        INT DEFAULT 0,
    max_input_length        INT DEFAULT 500,
    input_validation_regex  TEXT,
    redeem_prompt_text      TEXT, -- Prompt shown to users
    
    -- Management
    is_managed              BOOLEAN NOT NULL DEFAULT false, -- Bot manages reward on platform
    auto_fulfill            BOOLEAN NOT NULL DEFAULT true, -- Auto-mark as fulfilled
    
    -- Pipeline integration
    pipeline_id             UUID REFERENCES event_pipelines(pipeline_id), -- Execute pipeline on redemption
    
    -- Plugin integration
    plugin_name             TEXT,
    command_name            TEXT,
    
    -- Dynamic pricing
    dynamic_pricing         BOOLEAN NOT NULL DEFAULT false,
    min_cost                INT,
    max_cost                INT,
    cost_adjustment_percent FLOAT DEFAULT 0, -- Percentage to adjust cost after each use
    
    -- Metadata
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT platform_redeem_check CHECK (platform IN ('twitch', 'discord', 'vrchat')),
    CONSTRAINT redeem_unique UNIQUE(platform, reward_id)
);

CREATE INDEX idx_redeems_platform ON redeems(platform);
CREATE INDEX idx_redeems_active ON redeems(is_active);
CREATE INDEX idx_redeems_plugin ON redeems(plugin_name) WHERE plugin_name IS NOT NULL;

-- Redeem usage tracking (partitioned by time)
CREATE TABLE redeem_usage (
    usage_id        UUID NOT NULL DEFAULT uuid_generate_v4(),
    redeem_id       UUID NOT NULL REFERENCES redeems(redeem_id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    platform        TEXT NOT NULL,
    channel         TEXT,
    input_text      TEXT,
    cost_paid       INT NOT NULL, -- Actual cost at time of redemption
    status          TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'fulfilled', 'cancelled', 'refunded'
    fulfilled_by    UUID REFERENCES users(user_id),
    notes           TEXT,
    redeemed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at    TIMESTAMPTZ,
    
    PRIMARY KEY (usage_id, redeemed_at),
    CONSTRAINT status_redeem_check CHECK (
        status IN ('pending', 'fulfilled', 'cancelled', 'refunded')
    )
) PARTITION BY RANGE (redeemed_at);

CREATE INDEX idx_redeem_usage_redeem ON redeem_usage(redeem_id);
CREATE INDEX idx_redeem_usage_user ON redeem_usage(user_id);
CREATE INDEX idx_redeem_usage_status ON redeem_usage(status);
CREATE INDEX idx_redeem_usage_redeemed ON redeem_usage(redeemed_at DESC);

---------------------------------------------------------------------------
-- LINK REQUESTS (Platform account linking)
---------------------------------------------------------------------------

CREATE TABLE link_requests (
    link_request_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    from_user_id    UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    from_platform   TEXT NOT NULL,
    to_platform     TEXT NOT NULL,
    link_code       TEXT NOT NULL UNIQUE,
    status          TEXT NOT NULL DEFAULT 'pending',
    expires_at      TIMESTAMPTZ NOT NULL,
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT status_link_check CHECK (status IN ('pending', 'completed', 'expired', 'cancelled')),
    CONSTRAINT different_platforms CHECK (from_platform != to_platform)
);

CREATE INDEX idx_link_requests_code ON link_requests(link_code);
CREATE INDEX idx_link_requests_status ON link_requests(status);
CREATE INDEX idx_link_requests_expires ON link_requests(expires_at) WHERE status = 'pending';

---------------------------------------------------------------------------
-- INITIAL PARTITIONS FOR EVENT TABLES
---------------------------------------------------------------------------

-- Create initial partitions for usage tracking tables
DO $$
DECLARE
    current_month DATE := DATE_TRUNC('month', NOW());
    next_month DATE := DATE_TRUNC('month', NOW() + INTERVAL '1 month');
    next_next_month DATE := DATE_TRUNC('month', NOW() + INTERVAL '2 months');
BEGIN
    -- For command_usage
    EXECUTE format('CREATE TABLE IF NOT EXISTS command_usage_%s PARTITION OF command_usage FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(current_month, 'YYYYMM'), current_month, next_month);
    
    EXECUTE format('CREATE TABLE IF NOT EXISTS command_usage_%s PARTITION OF command_usage FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(next_month, 'YYYYMM'), next_month, next_next_month);
    
    -- For redeem_usage
    EXECUTE format('CREATE TABLE IF NOT EXISTS redeem_usage_%s PARTITION OF redeem_usage FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(current_month, 'YYYYMM'), current_month, next_month);
    
    EXECUTE format('CREATE TABLE IF NOT EXISTS redeem_usage_%s PARTITION OF redeem_usage FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(next_month, 'YYYYMM'), next_month, next_next_month);
        
    -- For pipeline_execution_log
    EXECUTE format('CREATE TABLE IF NOT EXISTS pipeline_execution_log_%s PARTITION OF pipeline_execution_log FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(current_month, 'YYYYMM'), current_month, next_month);
    
    EXECUTE format('CREATE TABLE IF NOT EXISTS pipeline_execution_log_%s PARTITION OF pipeline_execution_log FOR VALUES FROM (%L) TO (%L)',
        TO_CHAR(next_month, 'YYYYMM'), next_month, next_next_month);
END$$;

---------------------------------------------------------------------------
-- MIGRATION HELPERS
---------------------------------------------------------------------------

-- Function to migrate discord_event_config to pipelines
CREATE OR REPLACE FUNCTION migrate_discord_event_configs() RETURNS void AS $$
DECLARE
    config RECORD;
    new_pipeline_id UUID;
BEGIN
    FOR config IN SELECT * FROM discord_event_config LOOP
        -- Create pipeline for each event config
        INSERT INTO event_pipelines (name, description, enabled, priority)
        VALUES (
            'discord_' || config.event_name || '_' || config.channel_id,
            'Migrated from discord_event_config',
            true,
            100
        ) RETURNING pipeline_id INTO new_pipeline_id;
        
        -- Add platform filter
        INSERT INTO pipeline_filters (pipeline_id, filter_type, filter_config, filter_order)
        VALUES (
            new_pipeline_id,
            'platform_filter',
            jsonb_build_object('platforms', ARRAY['discord']),
            0
        );
        
        -- Add Discord message action
        INSERT INTO pipeline_actions (pipeline_id, action_type, action_config, action_order)
        VALUES (
            new_pipeline_id,
            'discord_message',
            jsonb_build_object(
                'channel_id', config.channel_id,
                'guild_id', config.guild_id,
                'credential_id', config.respond_with_credential,
                'ping_roles', config.ping_roles
            ),
            0
        );
        
        -- Update the discord_event_config to reference the new pipeline
        UPDATE discord_event_config 
        SET pipeline_id = new_pipeline_id 
        WHERE event_config_id = config.event_config_id;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

---------------------------------------------------------------------------
-- TRIGGERS
---------------------------------------------------------------------------

-- Update triggers for new tables
CREATE TRIGGER update_event_handler_registry_updated_at BEFORE UPDATE ON event_handler_registry
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_event_pipelines_updated_at BEFORE UPDATE ON event_pipelines
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_pipeline_filters_updated_at BEFORE UPDATE ON pipeline_filters
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_pipeline_actions_updated_at BEFORE UPDATE ON pipeline_actions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_commands_updated_at BEFORE UPDATE ON commands
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_redeems_updated_at BEFORE UPDATE ON redeems
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Function to update pipeline execution statistics
CREATE OR REPLACE FUNCTION update_pipeline_stats() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status IN ('success', 'failed') AND OLD.status = 'running' THEN
        UPDATE event_pipelines 
        SET 
            execution_count = execution_count + 1,
            success_count = CASE WHEN NEW.status = 'success' THEN success_count + 1 ELSE success_count END,
            last_executed = NEW.completed_at
        WHERE pipeline_id = NEW.pipeline_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_pipeline_stats_trigger AFTER UPDATE ON pipeline_execution_log
    FOR EACH ROW EXECUTE FUNCTION update_pipeline_stats();

---------------------------------------------------------------------------
-- ADD FOREIGN KEY CONSTRAINT
---------------------------------------------------------------------------

-- Now that event_pipelines table exists, add the foreign key constraint
ALTER TABLE chat_logging_config 
    ADD CONSTRAINT fk_chat_logging_pipeline 
    FOREIGN KEY (pre_drop_pipeline_id) 
    REFERENCES event_pipelines(pipeline_id) 
    ON DELETE SET NULL;
