-- File: migrations/006_ai_init.sql
-- AI Service Tables

---------------------------------------------------------------------------
-- ai_providers
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_providers CASCADE;
CREATE TABLE ai_providers (
    provider_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Ensure provider names are unique
CREATE UNIQUE INDEX ON ai_providers (LOWER(name));

---------------------------------------------------------------------------
-- ai_credentials
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_credentials CASCADE;
CREATE TABLE ai_credentials (
    credential_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider_id UUID NOT NULL REFERENCES ai_providers(provider_id) ON DELETE CASCADE,
    api_key TEXT NOT NULL,
    api_base TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    additional_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- No constraints on defaults, will be enforced through application logic

---------------------------------------------------------------------------
-- ai_models
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_models CASCADE;
CREATE TABLE ai_models (
    model_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider_id UUID NOT NULL REFERENCES ai_providers(provider_id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    capabilities JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Ensure model names are unique per provider
CREATE UNIQUE INDEX ON ai_models (provider_id, LOWER(name));
-- No constraints on defaults, will be enforced through application logic

---------------------------------------------------------------------------
-- ai_agents (MCPs)
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_agents CASCADE;
CREATE TABLE ai_agents (
    agent_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    description TEXT,
    model_id UUID NOT NULL REFERENCES ai_models(model_id) ON DELETE CASCADE,
    system_prompt TEXT,
    capabilities JSONB,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Ensure agent names are unique
CREATE UNIQUE INDEX ON ai_agents (LOWER(name));

---------------------------------------------------------------------------
-- ai_actions
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_actions CASCADE;
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
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Ensure action names are unique per agent
CREATE UNIQUE INDEX ON ai_actions (agent_id, LOWER(name));

---------------------------------------------------------------------------
-- ai_triggers
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_triggers CASCADE;
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
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (model_id IS NOT NULL OR agent_id IS NOT NULL)
);

-- Ensure no duplicate trigger patterns for same type
CREATE UNIQUE INDEX ON ai_triggers (trigger_type, LOWER(pattern));

---------------------------------------------------------------------------
-- ai_system_prompts
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_system_prompts CASCADE;
CREATE TABLE ai_system_prompts (
    prompt_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Ensure prompt names are unique
CREATE UNIQUE INDEX ON ai_system_prompts (LOWER(name));
-- No constraints on defaults, will be enforced through application logic

---------------------------------------------------------------------------
-- ai_memory
---------------------------------------------------------------------------
DROP TABLE IF EXISTS ai_memory CASCADE;
CREATE TABLE ai_memory (
    memory_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata JSONB
);

-- Index for faster memory lookup by user
CREATE INDEX ai_memory_user_timestamp_idx ON ai_memory (user_id, timestamp DESC);

---------------------------------------------------------------------------
-- Seed Data
---------------------------------------------------------------------------

-- Insert OpenAI provider
INSERT INTO ai_providers (provider_id, name, description)
VALUES (
    uuid_generate_v4(),
    'OpenAI',
    'OpenAI GPT models provider'
);

-- Insert Anthropic provider
INSERT INTO ai_providers (provider_id, name, description)
VALUES (
    uuid_generate_v4(),
    'Anthropic',
    'Anthropic Claude models provider'
);

-- Insert models for OpenAI
INSERT INTO ai_models (model_id, provider_id, name, description, is_default, capabilities)
VALUES (
    uuid_generate_v4(),
    (SELECT provider_id FROM ai_providers WHERE name = 'OpenAI'),
    'gpt-4o',
    'GPT-4o model with strong reasoning capabilities',
    true,
    '{"function_calling": true, "vision": true, "streaming": true}'
);

INSERT INTO ai_models (model_id, provider_id, name, description, capabilities)
VALUES (
    uuid_generate_v4(),
    (SELECT provider_id FROM ai_providers WHERE name = 'OpenAI'),
    'gpt-4.1',
    'Flagship model — well suited for problem solving across domains',
    '{"function_calling": true, "vision": false, "streaming": true}'
);

-- Insert models for Anthropic
INSERT INTO ai_models (model_id, provider_id, name, description, is_default, capabilities)
VALUES (
    uuid_generate_v4(),
    (SELECT provider_id FROM ai_providers WHERE name = 'Anthropic'),
    'claude-4-opus-20250514',
    'Most capable Claude model with strong reasoning',
    true,
    '{"function_calling": true, "vision": true, "streaming": true}'
);

INSERT INTO ai_models (model_id, provider_id, name, description, capabilities)
VALUES (
    uuid_generate_v4(),
    (SELECT provider_id FROM ai_providers WHERE name = 'Anthropic'),
    'claude-4-sonnet-20250514',
    'Balanced Claude model with good performance and cost',
    '{"function_calling": true, "vision": true, "streaming": true}'
);

-- Insert system prompts
INSERT INTO ai_system_prompts (prompt_id, name, content, description, is_default)
VALUES (
    uuid_generate_v4(),
    'Default Assistant',
    'You are Maow, a helpful AI assistant for a Twitch streamer. Respond to user queries in a friendly, helpful manner. Keep responses concise but informative.',
    'Default system prompt for general interactions',
    true
);

INSERT INTO ai_system_prompts (prompt_id, name, content, description)
VALUES (
    uuid_generate_v4(),
    'Twitch Chat Helper',
    'You are Maow, a helpful AI assistant for Twitch chat. Keep responses friendly, engaging, and brief (under 200 characters when possible). Avoid controversial topics.',
    'Optimized for Twitch chat responses'
);

-- Insert default agent (MCP)
INSERT INTO ai_agents (agent_id, name, description, model_id, system_prompt, capabilities, enabled)
VALUES (
    uuid_generate_v4(),
    'Maow Assistant',
    'Default assistant for handling general chat queries',
    (SELECT model_id FROM ai_models WHERE name = 'claude-4-sonnet-20250514'),
    'You are Maow, a helpful AI assistant that responds to user queries in a friendly manner.',
    '{"can_search": true, "can_remember": true, "max_response_tokens": 1000}',
    true
);

-- Insert example actions for the agent
INSERT INTO ai_actions (action_id, agent_id, name, description, input_schema, output_schema, handler_type, handler_config)
VALUES (
    uuid_generate_v4(),
    (SELECT agent_id FROM ai_agents WHERE name = 'Maow Assistant'),
    'get_stream_status',
    'Get the current status of the stream',
    '{}',
    '{"type": "object", "properties": {"is_live": {"type": "boolean"}, "uptime": {"type": "string"}, "viewers": {"type": "integer"}}}',
    'function',
    '{"plugin": "twitch", "method": "get_stream_status"}'
);

-- Insert default triggers
INSERT INTO ai_triggers (trigger_id, trigger_type, pattern, model_id, system_prompt, enabled)
VALUES (
    uuid_generate_v4(),
    'prefix',
    'hey maow',
    (SELECT model_id FROM ai_models WHERE name = 'claude-4-sonnet-20250514'),
    'You are Maow, a helpful AI assistant for a Twitch streamer. Respond to user queries in a friendly, helpful manner. Keep responses concise but informative.',
    true
);

INSERT INTO ai_triggers (trigger_id, trigger_type, pattern, model_id, system_prompt, enabled)
VALUES (
    uuid_generate_v4(),
    'prefix',
    '@maow',
    (SELECT model_id FROM ai_models WHERE name = 'claude-4-sonnet-20250514'),
    'You are Maow, a helpful AI assistant for a Twitch streamer. Respond to user queries in a friendly, helpful manner. Keep responses concise but informative.',
    true
);

-- Insert example trigger using an agent
INSERT INTO ai_triggers (trigger_id, trigger_type, pattern, agent_id, platform, channel, enabled)
VALUES (
    uuid_generate_v4(),
    'regex',
    'maow (help|info)',
    (SELECT agent_id FROM ai_agents WHERE name = 'Maow Assistant'),
    'twitch',
    'all',
    true
);
