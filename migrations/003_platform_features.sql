-- 003_platform_features.sql
-- Platform-specific features including Discord, AI, OSC, OBS, and Drip
-- Consolidated from original migrations 004, 005, 006, 008, 010, 011

-- Drop existing tables to ensure clean slate
DROP TABLE IF EXISTS 
    discord_event_config,
    discord_live_roles,
    discord_channels,
    discord_guilds,
    discord_accounts,
    ai_conversations,
    ai_message_history,
    ai_agents,
    ai_actions,
    ai_system_prompts,
    ai_triggers,
    osc_toggles,
    obs_instances,
    drip_feed_messages,
    drip_credits,
    drip_settings
CASCADE;

---------------------------------------------------------------------------
-- DISCORD FEATURES
---------------------------------------------------------------------------

-- Discord accounts (bot accounts)
CREATE TABLE discord_accounts (
    account_name    TEXT PRIMARY KEY,
    discord_id      TEXT UNIQUE,
    credential_id   UUID REFERENCES platform_credentials(credential_id) ON DELETE SET NULL,
    is_active       BOOLEAN NOT NULL DEFAULT FALSE,
    bot_token       TEXT, -- Encrypted in application
    application_id  TEXT,
    permissions     BIGINT DEFAULT 0, -- Discord permission integer
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Discord guilds (servers)
CREATE TABLE discord_guilds (
    account_name    TEXT NOT NULL REFERENCES discord_accounts(account_name) ON DELETE CASCADE,
    guild_id        TEXT NOT NULL,
    guild_name      TEXT NOT NULL,
    icon_hash       TEXT,
    owner_id        TEXT,
    member_count    INT,
    is_active       BOOLEAN NOT NULL DEFAULT FALSE,
    permissions     BIGINT, -- Bot's permissions in this guild
    joined_at       TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT pk_discord_guilds PRIMARY KEY (account_name, guild_id)
);

CREATE INDEX idx_discord_guilds_active ON discord_guilds(is_active);

-- Discord channels
CREATE TABLE discord_channels (
    account_name    TEXT NOT NULL,
    guild_id        TEXT NOT NULL,
    channel_id      TEXT NOT NULL,
    channel_name    TEXT NOT NULL,
    channel_type    INT NOT NULL DEFAULT 0, -- Discord channel type enum
    position        INT,
    topic           TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT FALSE,
    can_send        BOOLEAN NOT NULL DEFAULT TRUE, -- Bot can send messages
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT pk_discord_channels PRIMARY KEY (account_name, guild_id, channel_id),
    CONSTRAINT fk_discord_guilds FOREIGN KEY (account_name, guild_id)
        REFERENCES discord_guilds (account_name, guild_id) ON DELETE CASCADE
);

CREATE INDEX idx_discord_channels_active ON discord_channels(is_active);
CREATE INDEX idx_discord_channels_type ON discord_channels(channel_type);

-- Discord event configuration (now integrated with pipelines)
CREATE TABLE discord_event_config (
    event_config_id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_name              TEXT NOT NULL,
    guild_id                TEXT NOT NULL,
    channel_id              TEXT NOT NULL,
    respond_with_credential UUID REFERENCES platform_credentials(credential_id),
    ping_roles              TEXT[],
    
    -- Pipeline integration
    pipeline_id             UUID REFERENCES event_pipelines(pipeline_id),
    
    -- Legacy support during migration
    is_migrated             BOOLEAN NOT NULL DEFAULT FALSE,
    
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT discord_event_config_unique UNIQUE (event_name, guild_id, channel_id)
);

CREATE INDEX idx_discord_event_name ON discord_event_config(event_name);
CREATE INDEX idx_discord_event_pipeline ON discord_event_config(pipeline_id) WHERE pipeline_id IS NOT NULL;

-- Discord live roles (assign role when streaming)
CREATE TABLE discord_live_roles (
    live_role_id    UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    guild_id        TEXT NOT NULL,
    role_id         TEXT NOT NULL,
    role_name       TEXT, -- For display purposes
    required_game   TEXT, -- Optional: Only assign if streaming specific game
    min_viewers     INT DEFAULT 0, -- Optional: Minimum viewers to assign role
    
    -- Pipeline integration for advanced logic
    pipeline_id     UUID REFERENCES event_pipelines(pipeline_id),
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_live_role_per_guild UNIQUE(guild_id)
);

---------------------------------------------------------------------------
-- AI SYSTEM
---------------------------------------------------------------------------

-- First, create the tables that the AI repository expects
-- These maintain compatibility with existing code

-- AI providers
CREATE TABLE ai_providers (
    provider_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX ON ai_providers (LOWER(name));

-- AI credentials
CREATE TABLE ai_credentials (
    credential_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider_id UUID NOT NULL REFERENCES ai_providers(provider_id) ON DELETE CASCADE,
    api_key TEXT NOT NULL, -- Encrypted in application
    api_base TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    additional_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- AI models
CREATE TABLE ai_models (
    model_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider_id UUID NOT NULL REFERENCES ai_providers(provider_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    capabilities JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX ON ai_models (provider_id, LOWER(name));

-- AI system prompts (kept from new design)
CREATE TABLE ai_system_prompts (
    prompt_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX ON ai_system_prompts (LOWER(name));

-- AI agents (hybrid of old and new design)
CREATE TABLE ai_agents (
    agent_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    description TEXT,
    model_id UUID NOT NULL REFERENCES ai_models(model_id) ON DELETE CASCADE,
    system_prompt TEXT,
    capabilities JSONB,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX ON ai_agents (LOWER(name));

-- AI actions (kept from old design for compatibility)
CREATE TABLE ai_actions (
    action_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES ai_agents(agent_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    input_schema JSONB,
    output_schema JSONB,
    handler_type TEXT NOT NULL, -- 'function', 'plugin', 'webhook', etc.
    handler_config JSONB,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX ON ai_actions (agent_id, LOWER(name));

-- AI triggers (kept from old design)
CREATE TABLE ai_triggers (
    trigger_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    trigger_type TEXT NOT NULL,
    pattern TEXT NOT NULL,
    model_id UUID REFERENCES ai_models(model_id) ON DELETE CASCADE,
    agent_id UUID REFERENCES ai_agents(agent_id) ON DELETE CASCADE,
    system_prompt TEXT,
    platform TEXT,
    channel TEXT,
    schedule TEXT,
    condition TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (model_id IS NOT NULL OR agent_id IS NOT NULL)
);

CREATE UNIQUE INDEX ON ai_triggers (trigger_type, LOWER(pattern));

-- AI memory (enhanced for personality/robotics)
CREATE TABLE ai_memory (
    memory_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB,
    -- New fields for personality system
    personality_id UUID, -- Will reference ai_personalities when created
    memory_type TEXT DEFAULT 'conversation', -- 'conversation', 'fact', 'preference', 'relationship'
    importance FLOAT DEFAULT 0.5, -- For memory pruning
    emotional_context JSONB, -- Emotions associated with memory
    accessed_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ -- Some memories can fade
);

CREATE INDEX ai_memory_user_timestamp_idx ON ai_memory (user_id, timestamp DESC);
CREATE INDEX ai_memory_personality_idx ON ai_memory (personality_id) WHERE personality_id IS NOT NULL;
CREATE INDEX ai_memory_type_idx ON ai_memory (memory_type);

-- AI configurations (new table for service-level config)
CREATE TABLE ai_configurations (
    config_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider TEXT NOT NULL, -- 'openai', 'anthropic', 'local'
    api_key TEXT, -- Encrypted
    api_endpoint TEXT,
    model_settings JSONB DEFAULT '{}', -- Model-specific settings
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

---------------------------------------------------------------------------
-- AI PERSONALITY SYSTEM (for future robotics/VRChat integration)
---------------------------------------------------------------------------

-- AI personalities for VRChat/robotics
CREATE TABLE ai_personalities (
    personality_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT UNIQUE NOT NULL,
    base_prompt TEXT NOT NULL,
    voice_config JSONB DEFAULT '{}', -- TTS settings
    avatar_config JSONB DEFAULT '{}', -- VRChat avatar preferences
    personality_traits JSONB DEFAULT '{}', -- Traits that affect responses
    emotional_ranges JSONB DEFAULT '{}', -- Min/max for emotions
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- AI personality state (mood, energy, etc.)
CREATE TABLE ai_personality_state (
    personality_id UUID REFERENCES ai_personalities(personality_id) PRIMARY KEY,
    current_mood JSONB DEFAULT '{}', -- {happiness: 0.7, energy: 0.5, etc.}
    current_activity TEXT, -- What the AI is "doing"
    last_interaction TIMESTAMPTZ,
    interaction_count BIGINT DEFAULT 0,
    avatar_position JSONB, -- For VRChat position tracking
    avatar_state JSONB, -- Current avatar parameters
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Link personalities to agents for backward compatibility
ALTER TABLE ai_agents ADD COLUMN personality_id UUID REFERENCES ai_personalities(personality_id);

---------------------------------------------------------------------------
-- OSC (OPEN SOUND CONTROL) SYSTEM
---------------------------------------------------------------------------

-- OSC toggle configurations
CREATE TABLE osc_toggles (
    toggle_id       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    internal_name   TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    description     TEXT,
    
    -- OSC configuration
    osc_address     TEXT NOT NULL, -- e.g., "/avatar/parameters/toggle_name"
    value_type      TEXT NOT NULL DEFAULT 'bool', -- 'bool', 'int', 'float'
    default_value   JSONB NOT NULL DEFAULT 'false',
    active_value    JSONB NOT NULL DEFAULT 'true',
    
    -- Timing configuration
    duration_ms     INT, -- How long to stay active (null = permanent)
    fade_in_ms      INT DEFAULT 0,
    fade_out_ms     INT DEFAULT 0,
    
    -- Trigger configuration
    trigger_type    TEXT NOT NULL DEFAULT 'manual', -- 'manual', 'redeem', 'command', 'event'
    trigger_config  JSONB DEFAULT '{}',
    
    -- Limits
    cooldown_ms     INT DEFAULT 0,
    max_concurrent  INT DEFAULT 1, -- How many can be active at once
    
    -- State tracking
    is_active       BOOLEAN NOT NULL DEFAULT FALSE,
    last_triggered  TIMESTAMPTZ,
    trigger_count   BIGINT NOT NULL DEFAULT 0,
    
    -- Pipeline integration
    pipeline_id     UUID REFERENCES event_pipelines(pipeline_id),
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT value_type_check CHECK (value_type IN ('bool', 'int', 'float')),
    CONSTRAINT trigger_type_osc_check CHECK (trigger_type IN ('manual', 'redeem', 'command', 'event', 'schedule'))
);

CREATE INDEX idx_osc_toggles_active ON osc_toggles(is_active);
CREATE INDEX idx_osc_toggles_trigger_type ON osc_toggles(trigger_type);

---------------------------------------------------------------------------
-- OBS INTEGRATION
---------------------------------------------------------------------------

-- OBS instances (multiple OBS connections)
CREATE TABLE obs_instances (
    instance_id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_name   TEXT NOT NULL UNIQUE,
    
    -- Connection details
    host            TEXT NOT NULL DEFAULT 'localhost',
    port            INT NOT NULL DEFAULT 4455,
    use_password    BOOLEAN NOT NULL DEFAULT FALSE,
    password        TEXT, -- Encrypted in application
    
    -- State
    is_connected    BOOLEAN NOT NULL DEFAULT FALSE,
    is_enabled      BOOLEAN NOT NULL DEFAULT TRUE,
    last_connected  TIMESTAMPTZ,
    
    -- Capabilities (discovered from OBS)
    obs_version     TEXT,
    websocket_version TEXT,
    available_requests TEXT[],
    
    -- Auto-connect settings
    auto_connect    BOOLEAN NOT NULL DEFAULT TRUE,
    reconnect_delay_ms INT DEFAULT 5000,
    max_reconnect_attempts INT DEFAULT 10,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_obs_instances_enabled ON obs_instances(is_enabled);

---------------------------------------------------------------------------
-- DRIP FEED SYSTEM
---------------------------------------------------------------------------

-- Drip settings (global configuration)
CREATE TABLE drip_settings (
    setting_key     TEXT PRIMARY KEY,
    setting_value   JSONB NOT NULL,
    description     TEXT,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Default drip settings
INSERT INTO drip_settings (setting_key, setting_value, description) VALUES
    ('drip_interval_minutes', '5', 'Minutes between drip messages'),
    ('drip_amount', '10', 'Credits awarded per drip'),
    ('drip_enabled', 'true', 'Whether drip system is active'),
    ('max_drips_per_stream', '50', 'Maximum drips per stream session');

-- Drip credits tracking
CREATE TABLE drip_credits (
    credit_id       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id         UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    platform        TEXT NOT NULL,
    
    -- Credit balance
    current_balance BIGINT NOT NULL DEFAULT 0,
    total_earned    BIGINT NOT NULL DEFAULT 0,
    total_spent     BIGINT NOT NULL DEFAULT 0,
    
    -- Activity tracking
    last_drip_at    TIMESTAMPTZ,
    drip_count      INT NOT NULL DEFAULT 0,
    streak_count    INT NOT NULL DEFAULT 0,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT drip_credits_unique UNIQUE(user_id, platform),
    CONSTRAINT balance_not_negative CHECK (current_balance >= 0)
);

CREATE INDEX idx_drip_credits_user ON drip_credits(user_id);
CREATE INDEX idx_drip_credits_balance ON drip_credits(current_balance) WHERE current_balance > 0;

-- Drip feed messages
CREATE TABLE drip_feed_messages (
    message_id      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    message_text    TEXT NOT NULL,
    message_type    TEXT NOT NULL DEFAULT 'standard', -- 'standard', 'bonus', 'special'
    
    -- Weights and probability
    weight          INT NOT NULL DEFAULT 100, -- Higher weight = more likely
    
    -- Requirements
    min_watch_time_minutes INT DEFAULT 0,
    min_message_count INT DEFAULT 0,
    required_roles  TEXT[] DEFAULT '{}',
    
    -- Bonus configuration
    bonus_multiplier FLOAT DEFAULT 1.0,
    bonus_flat_amount INT DEFAULT 0,
    
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    use_count       BIGINT NOT NULL DEFAULT 0,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT message_type_check CHECK (message_type IN ('standard', 'bonus', 'special', 'milestone'))
);

CREATE INDEX idx_drip_messages_active ON drip_feed_messages(is_active);
CREATE INDEX idx_drip_messages_type ON drip_feed_messages(message_type);

---------------------------------------------------------------------------
-- TRIGGERS
---------------------------------------------------------------------------

-- Add update triggers for all new tables
CREATE TRIGGER update_discord_accounts_updated_at BEFORE UPDATE ON discord_accounts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_discord_guilds_updated_at BEFORE UPDATE ON discord_guilds
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_discord_channels_updated_at BEFORE UPDATE ON discord_channels
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_discord_event_config_updated_at BEFORE UPDATE ON discord_event_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_discord_live_roles_updated_at BEFORE UPDATE ON discord_live_roles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- AI table triggers
CREATE TRIGGER update_ai_providers_updated_at BEFORE UPDATE ON ai_providers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_credentials_updated_at BEFORE UPDATE ON ai_credentials
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_models_updated_at BEFORE UPDATE ON ai_models
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_system_prompts_updated_at BEFORE UPDATE ON ai_system_prompts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_agents_updated_at BEFORE UPDATE ON ai_agents
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_triggers_updated_at BEFORE UPDATE ON ai_triggers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_actions_updated_at BEFORE UPDATE ON ai_actions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_configurations_updated_at BEFORE UPDATE ON ai_configurations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_personalities_updated_at BEFORE UPDATE ON ai_personalities
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_personality_state_updated_at BEFORE UPDATE ON ai_personality_state
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- OSC and OBS triggers
CREATE TRIGGER update_osc_toggles_updated_at BEFORE UPDATE ON osc_toggles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_obs_instances_updated_at BEFORE UPDATE ON obs_instances
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Drip system triggers
CREATE TRIGGER update_drip_settings_updated_at BEFORE UPDATE ON drip_settings
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_drip_credits_updated_at BEFORE UPDATE ON drip_credits
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_drip_feed_messages_updated_at BEFORE UPDATE ON drip_feed_messages
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();