-- 004_seed_data.sql
-- Seed data including built-in commands, redeems, event handlers, and example pipelines

---------------------------------------------------------------------------
-- EVENT TYPE REGISTRY
---------------------------------------------------------------------------

-- Twitch event types
INSERT INTO event_type_registry (platform, event_category, event_name, description) VALUES
    -- Chat events
    ('twitch-irc', 'chat', 'message.create', 'Chat message received'),
    ('twitch-irc', 'chat', 'message.delete', 'Chat message deleted'),
    ('twitch-irc', 'chat', 'user.timeout', 'User timed out'),
    ('twitch-irc', 'chat', 'user.ban', 'User banned'),
    
    -- Stream events
    ('twitch-eventsub', 'stream', 'stream.online', 'Stream went online'),
    ('twitch-eventsub', 'stream', 'stream.offline', 'Stream went offline'),
    ('twitch-eventsub', 'stream', 'channel.update', 'Channel information updated'),
    
    -- Follow/Subscribe events
    ('twitch-eventsub', 'user', 'channel.follow', 'New follower'),
    ('twitch-eventsub', 'subscription', 'channel.subscribe', 'New subscription'),
    ('twitch-eventsub', 'subscription', 'channel.subscription.gift', 'Gifted subscriptions'),
    ('twitch-eventsub', 'subscription', 'channel.subscription.message', 'Resubscription with message'),
    
    -- Channel points
    ('twitch-eventsub', 'points', 'channel.channel_points_custom_reward_redemption.add', 'Channel points redeemed'),
    
    -- Raids and hosts
    ('twitch-eventsub', 'raid', 'channel.raid', 'Channel raided'),
    
    -- Bits
    ('twitch-eventsub', 'bits', 'channel.cheer', 'Bits cheered'),
    
    -- Moderation
    ('twitch-eventsub', 'moderation', 'channel.moderator.add', 'Moderator added'),
    ('twitch-eventsub', 'moderation', 'channel.moderator.remove', 'Moderator removed');

-- Discord event types
INSERT INTO event_type_registry (platform, event_category, event_name, description) VALUES
    -- Messages
    ('discord', 'chat', 'message.create', 'Message sent'),
    ('discord', 'chat', 'message.update', 'Message edited'),
    ('discord', 'chat', 'message.delete', 'Message deleted'),
    
    -- Member events
    ('discord', 'member', 'member.join', 'Member joined server'),
    ('discord', 'member', 'member.leave', 'Member left server'),
    ('discord', 'member', 'member.update', 'Member updated (roles, nickname, etc)'),
    
    -- Voice events
    ('discord', 'voice', 'voice.join', 'Member joined voice channel'),
    ('discord', 'voice', 'voice.leave', 'Member left voice channel'),
    ('discord', 'voice', 'voice.move', 'Member moved voice channels'),
    
    -- Presence
    ('discord', 'presence', 'presence.update', 'Member presence updated (status, activity)'),
    
    -- Interactions
    ('discord', 'interaction', 'interaction.create', 'Slash command or button interaction'),
    
    -- Bot events
    ('discord', 'bot', 'ready', 'Bot connected and ready');

-- System events
INSERT INTO event_type_registry (platform, event_category, event_name, description) VALUES
    ('system', 'lifecycle', 'startup', 'Bot started'),
    ('system', 'lifecycle', 'shutdown', 'Bot shutting down'),
    ('system', 'plugin', 'plugin.loaded', 'Plugin loaded'),
    ('system', 'plugin', 'plugin.unloaded', 'Plugin unloaded'),
    ('system', 'error', 'error.critical', 'Critical error occurred');

---------------------------------------------------------------------------
-- EVENT HANDLER REGISTRY
---------------------------------------------------------------------------

-- Built-in filters
INSERT INTO event_handler_registry (handler_type, handler_name, handler_category, description, parameters, is_builtin) VALUES
    -- Platform filters
    ('filter', 'platform_filter', 'platform', 'Filter by platform', 
     '{"platforms": {"type": "array", "items": "string", "required": true}}', true),
    
    -- Channel filters
    ('filter', 'channel_filter', 'channel', 'Filter by channel name',
     '{"channels": {"type": "array", "items": "string", "required": true}}', true),
    
    -- User filters
    ('filter', 'user_role_filter', 'user', 'Filter by user roles',
     '{"roles": {"type": "array", "items": "string", "required": true}, "match_any": {"type": "boolean", "default": true}}', true),
    
    ('filter', 'user_level_filter', 'user', 'Filter by user level',
     '{"min_level": {"type": "string", "enum": ["viewer", "subscriber", "vip", "moderator", "broadcaster"], "required": true}}', true),
    
    -- Message filters
    ('filter', 'message_pattern_filter', 'message', 'Filter by message pattern',
     '{"patterns": {"type": "array", "items": "string", "required": true}, "match_any": {"type": "boolean", "default": true}}', true),
    
    ('filter', 'message_length_filter', 'message', 'Filter by message length',
     '{"min_length": {"type": "integer", "default": 0}, "max_length": {"type": "integer", "default": 500}}', true),
    
    -- Time filters
    ('filter', 'time_window_filter', 'time', 'Filter by time of day',
     '{"start_hour": {"type": "integer", "min": 0, "max": 23}, "end_hour": {"type": "integer", "min": 0, "max": 23}, "timezone": {"type": "string", "default": "UTC"}}', true),
    
    ('filter', 'cooldown_filter', 'time', 'Rate limit filter',
     '{"cooldown_seconds": {"type": "integer", "required": true}, "per_user": {"type": "boolean", "default": true}}', true);

-- Built-in actions
INSERT INTO event_handler_registry (handler_type, handler_name, handler_category, description, parameters, is_builtin) VALUES
    -- Logging
    ('action', 'log_action', 'system', 'Log event details',
     '{"level": {"type": "string", "enum": ["error", "warn", "info", "debug", "trace"], "default": "info"}}', true),
    
    -- Discord actions
    ('action', 'discord_message', 'discord', 'Send Discord message',
     '{"channel_id": {"type": "string", "required": true}, "message": {"type": "string", "required": true}, "embed": {"type": "object"}}', true),
    
    ('action', 'discord_role_add', 'discord', 'Add Discord role',
     '{"role_id": {"type": "string", "required": true}, "user_id": {"type": "string", "required": true}}', true),
    
    ('action', 'discord_role_remove', 'discord', 'Remove Discord role',
     '{"role_id": {"type": "string", "required": true}, "user_id": {"type": "string", "required": true}}', true),
    
    -- Twitch actions
    ('action', 'twitch_message', 'twitch', 'Send Twitch chat message',
     '{"message": {"type": "string", "required": true}, "reply_to": {"type": "string"}}', true),
    
    ('action', 'twitch_timeout', 'twitch', 'Timeout user on Twitch',
     '{"user_id": {"type": "string", "required": true}, "duration": {"type": "integer", "default": 600}, "reason": {"type": "string"}}', true),
    
    -- OSC actions
    ('action', 'osc_trigger', 'osc', 'Trigger OSC parameter',
     '{"address": {"type": "string", "required": true}, "value": {"type": "any", "required": true}, "duration_ms": {"type": "integer"}}', true),
    
    -- OBS actions
    ('action', 'obs_scene_change', 'obs', 'Change OBS scene',
     '{"scene_name": {"type": "string", "required": true}, "instance": {"type": "string", "default": "default"}}', true),
    
    ('action', 'obs_source_toggle', 'obs', 'Toggle OBS source visibility',
     '{"source_name": {"type": "string", "required": true}, "visible": {"type": "boolean", "required": true}, "scene": {"type": "string"}}', true),
    
    -- Plugin actions
    ('action', 'plugin_call', 'plugin', 'Call plugin function',
     '{"plugin_id": {"type": "string", "required": true}, "function": {"type": "string", "required": true}, "parameters": {"type": "object"}}', true),
    
    -- AI actions
    ('action', 'ai_respond', 'ai', 'Generate AI response',
     '{"agent_id": {"type": "string", "required": true}, "prompt_template": {"type": "string"}}', true),
    
    -- AI Response Actions
    ('action', 'ai_text_response', 'ai', 'Generate text response with personality', 
     '{"personality_id": {"type": "string", "required": true}, "include_memory": {"type": "boolean", "default": true}, "emotion_influence": {"type": "number", "default": 0.5, "min": 0, "max": 1}}', true),
    
    ('action', 'ai_voice_response', 'ai', 'Generate voice response with TTS',
     '{"personality_id": {"type": "string", "required": true}, "voice_emotion": {"type": "string", "default": "auto"}, "speed": {"type": "number", "default": 1.0, "min": 0.5, "max": 2.0}}', true),
    
    ('action', 'ai_analyze_input', 'ai', 'Analyze input for intent and emotion',
     '{"analyze_emotion": {"type": "boolean", "default": true}, "analyze_intent": {"type": "boolean", "default": true}, "update_mood": {"type": "boolean", "default": true}}', true),
    
    -- Robotics/Avatar Actions  
    ('action', 'avatar_set_emotion', 'robotics', 'Set avatar emotional expression',
     '{"emotion": {"type": "string", "required": true}, "intensity": {"type": "number", "default": 1.0, "min": 0, "max": 1}, "duration_ms": {"type": "integer", "default": 5000}}', true),
    
    ('action', 'avatar_perform_gesture', 'robotics', 'Trigger avatar gesture or animation',
     '{"gesture": {"type": "string", "required": true}, "loop": {"type": "boolean", "default": false}, "blend_time": {"type": "integer", "default": 500}}', true),
    
    ('action', 'avatar_move_to', 'robotics', 'Move avatar in VRChat world',
     '{"position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3}, "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3}, "speed": {"type": "number", "default": 1.0}}', true),
    
    ('action', 'avatar_interact', 'robotics', 'Interact with VRChat world object',
     '{"object_id": {"type": "string", "required": true}, "interaction_type": {"type": "string", "enum": ["touch", "grab", "activate"], "required": true}}', true),
    
    -- Memory Actions
    ('action', 'ai_remember', 'ai', 'Store information in AI memory',
     '{"memory_type": {"type": "string", "default": "conversation"}, "importance": {"type": "number", "default": 0.5, "min": 0, "max": 1}, "auto_expire": {"type": "boolean", "default": true}}', true),
    
    ('action', 'ai_recall', 'ai', 'Retrieve relevant memories',
     '{"context": {"type": "string", "required": true}, "limit": {"type": "integer", "default": 10}, "min_importance": {"type": "number", "default": 0.3, "min": 0, "max": 1}}', true),
    
    -- Mood/State Actions
    ('action', 'ai_update_mood', 'ai', 'Update AI personality mood',
     '{"mood_delta": {"type": "object", "required": true}, "reason": {"type": "string"}, "decay_rate": {"type": "number", "default": 0.1, "min": 0, "max": 1}}', true);

---------------------------------------------------------------------------
-- BUILT-IN COMMANDS
---------------------------------------------------------------------------

-- Basic Twitch commands
INSERT INTO commands (
    platform, command_name, min_role, is_active, 
    default_response, plugin_name
) VALUES
    ('twitch', 'ping', 'viewer', true, 'Pong! 🏓', 'builtin'),
    ('twitch', 'uptime', 'viewer', true, 'Stream has been live for {uptime}', 'builtin'),
    ('twitch', 'followage', 'viewer', true, '@{user} has been following for {followage}', 'builtin'),
    ('twitch', 'lurk', 'viewer', true, '{user} is now lurking in the shadows... 👻', 'builtin'),
    ('twitch', 'unlurk', 'viewer', true, 'Welcome back {user}! 👋', 'builtin'),
    ('twitch', 'hug', 'viewer', true, '{user} gives {target} a warm hug! 🤗', 'builtin'),
    ('twitch', 'discord', 'viewer', true, 'Join our Discord: {discord_link}', 'builtin'),
    ('twitch', 'socials', 'viewer', true, 'Follow me on: {social_links}', 'builtin');

-- Twitch moderator commands
INSERT INTO commands (
    platform, command_name, min_role, is_active, plugin_name
) VALUES
    ('twitch', 'vanish', 'moderator', true, 'builtin'),
    ('twitch', 'so', 'moderator', true, 'builtin'),
    ('twitch', 'raid', 'moderator', true, 'builtin'),
    ('twitch', 'title', 'moderator', true, 'builtin'),
    ('twitch', 'game', 'moderator', true, 'builtin');

-- Twitch broadcaster commands
INSERT INTO commands (
    platform, command_name, min_role, is_active, plugin_name
) VALUES
    ('twitch', 'commercial', 'broadcaster', true, 'builtin'),
    ('twitch', 'marker', 'broadcaster', true, 'builtin'),
    ('twitch', 'prediction', 'broadcaster', true, 'builtin');

-- Discord commands
INSERT INTO commands (
    platform, command_name, min_role, is_active, 
    default_response, plugin_name
) VALUES
    ('discord', 'ping', 'viewer', true, 'Pong! 🏓', 'builtin'),
    ('discord', 'help', 'viewer', true, 'Use `/help` to see available commands', 'builtin'),
    ('discord', 'serverinfo', 'viewer', true, 'Server info: {server_info}', 'builtin');

---------------------------------------------------------------------------
-- BUILT-IN REDEEMS
---------------------------------------------------------------------------

-- Basic redeems
INSERT INTO redeems (
    platform, reward_id, reward_name, internal_name,
    cost, is_active, plugin_name, command_name
) VALUES
    ('twitch', 'builtin_cute', 'Make Me Cute', 'cute', 500, true, 'builtin', 'cute'),
    ('twitch', 'builtin_askai', 'Ask AI', 'askai', 1000, true, 'builtin', 'askai'),
    ('twitch', 'builtin_osc_happy', 'Happy Dance', 'osc_happy', 250, true, 'builtin', 'osc_trigger'),
    ('twitch', 'builtin_osc_sad', 'Sad Face', 'osc_sad', 250, true, 'builtin', 'osc_trigger'),
    ('twitch', 'builtin_hydrate', 'Hydrate Reminder', 'hydrate', 100, true, 'builtin', 'hydrate'),
    ('twitch', 'builtin_stretch', 'Stretch Reminder', 'stretch', 100, true, 'builtin', 'stretch');

-- Premium redeems
INSERT INTO redeems (
    platform, reward_id, reward_name, internal_name,
    cost, is_active, is_input_required, max_input_length,
    plugin_name, command_name
) VALUES
    ('twitch', 'builtin_tts', 'Text to Speech', 'tts', 2000, true, true, 200, 'builtin', 'tts'),
    ('twitch', 'builtin_song_request', 'Song Request', 'songrequest', 1500, true, true, 100, 'builtin', 'songrequest'),
    ('twitch', 'builtin_vip_day', 'VIP for a Day', 'vipday', 50000, true, false, 0, 'builtin', 'vipday');

---------------------------------------------------------------------------
-- EXAMPLE PIPELINES
---------------------------------------------------------------------------

-- Stream online announcement pipeline
INSERT INTO event_pipelines (name, description, enabled, priority, stop_on_match) VALUES
    ('stream_online_announcement', 'Announce when stream goes online', true, 10, false);

-- Add filters for stream online pipeline
INSERT INTO pipeline_filters (pipeline_id, filter_type, filter_config, filter_order) VALUES
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'stream_online_announcement'),
     'platform_filter', '{"platforms": ["twitch-eventsub"]}', 0);

-- Add actions for stream online pipeline
INSERT INTO pipeline_actions (pipeline_id, action_type, action_config, action_order) VALUES
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'stream_online_announcement'),
     'discord_message', 
     '{"account": "default", "channel_id": "CONFIGURE_ME", "message_template": "🔴 **{broadcaster}** is now live on Twitch!\\n\\n{title}\\nPlaying: {game}\\n\\nhttps://twitch.tv/{broadcaster}"}',
     0),
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'stream_online_announcement'),
     'osc_trigger',
     '{"parameter_path": "/avatar/parameters/streaming", "value": 1.0}',
     1);

-- Chat command pipeline
INSERT INTO event_pipelines (name, description, enabled, priority) VALUES
    ('chat_commands', 'Process chat commands', true, 50);

INSERT INTO pipeline_filters (pipeline_id, filter_type, filter_config, filter_order) VALUES
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'chat_commands'),
     'platform_filter', '{"platforms": ["twitch-irc", "discord"]}', 0),
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'chat_commands'),
     'message_pattern_filter', '{"patterns": ["^!"], "match_any": false}', 1);

-- Auto-moderation pipeline
INSERT INTO event_pipelines (name, description, enabled, priority, stop_on_match) VALUES
    ('auto_moderation', 'Automatic chat moderation', true, 1, true);

INSERT INTO pipeline_filters (pipeline_id, filter_type, filter_config, filter_order) VALUES
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'auto_moderation'),
     'platform_filter', '{"platforms": ["twitch-irc"]}', 0),
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'auto_moderation'),
     'message_pattern_filter', 
     '{"patterns": ["(?i)buy.*followers", "(?i)bit\\.ly", "(?i)discord\\.gg"], "match_any": true}', 1);

INSERT INTO pipeline_actions (pipeline_id, action_type, action_config, action_order) VALUES
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'auto_moderation'),
     'twitch_timeout',
     '{"account": "default", "duration": 600, "reason": "Automated: Suspicious link detected"}', 0),
    ((SELECT pipeline_id FROM event_pipelines WHERE name = 'auto_moderation'),
     'log_action',
     '{"level": "warn"}', 1);

---------------------------------------------------------------------------
-- AI SYSTEM SEED DATA
---------------------------------------------------------------------------

-- Insert AI providers
INSERT INTO ai_providers (name, description, enabled) VALUES
    ('OpenAI', 'OpenAI GPT models provider', true),
    ('Anthropic', 'Anthropic Claude models provider', true),
    ('Local', 'Local AI models (Ollama, etc)', true);

-- Insert AI models
INSERT INTO ai_models (provider_id, name, description, is_default, capabilities) VALUES
    -- OpenAI models
    ((SELECT provider_id FROM ai_providers WHERE name = 'OpenAI'), 
     'gpt-4o', 'GPT-4o model with strong reasoning capabilities', true,
     '{"function_calling": true, "vision": true, "streaming": true, "max_tokens": 128000}'),
    
    ((SELECT provider_id FROM ai_providers WHERE name = 'OpenAI'), 
     'gpt-4o-mini', 'Smaller, faster GPT-4o variant', false,
     '{"function_calling": true, "vision": true, "streaming": true, "max_tokens": 128000}'),
    
    ((SELECT provider_id FROM ai_providers WHERE name = 'OpenAI'), 
     'gpt-3.5-turbo', 'Fast and efficient model for simple tasks', false,
     '{"function_calling": true, "vision": false, "streaming": true, "max_tokens": 16385}'),
    
    -- Anthropic models
    ((SELECT provider_id FROM ai_providers WHERE name = 'Anthropic'), 
     'claude-3-5-sonnet-20241022', 'Most capable Claude model', true,
     '{"function_calling": true, "vision": true, "streaming": true, "max_tokens": 200000}'),
    
    ((SELECT provider_id FROM ai_providers WHERE name = 'Anthropic'), 
     'claude-3-5-haiku-20241022', 'Fast Claude model for simple tasks', false,
     '{"function_calling": true, "vision": true, "streaming": true, "max_tokens": 200000}');

-- Default AI prompts
INSERT INTO ai_system_prompts (name, content, description, is_default) VALUES
    ('Default Assistant', 
     'You are Maow, a helpful AI assistant for a Twitch streamer. Respond to user queries in a friendly, helpful manner. Keep responses concise but informative.',
     'Default system prompt for general interactions', 
     true),
    
    ('Twitch Chat Helper',
     'You are Maow, a helpful AI assistant for Twitch chat. Keep responses friendly, engaging, and brief (under 200 characters when possible). Avoid controversial topics.',
     'Optimized for Twitch chat responses',
     false),
    
    ('Gaming Buddy', 
     'You are a knowledgeable gaming companion. Provide tips, celebrate achievements, and engage in gaming discussions. Stay positive and encouraging!',
     'Gaming-focused assistant',
     false),
    
    ('Creative Helper', 
     'You are a creative assistant helping with art, music, and creative projects. Be encouraging and offer constructive feedback.',
     'Creative projects assistant',
     false);

-- Default AI agent
INSERT INTO ai_agents (name, description, model_id, system_prompt, capabilities, enabled) VALUES
    ('Maow Assistant', 
     'Default assistant for handling general chat queries',
     (SELECT model_id FROM ai_models WHERE name = 'claude-3-5-sonnet-20241022'),
     'You are Maow, a helpful AI assistant that responds to user queries in a friendly manner.',
     '{"can_search": true, "can_remember": true, "max_response_tokens": 1000}',
     true);

-- AI triggers
INSERT INTO ai_triggers (trigger_type, pattern, model_id, system_prompt, enabled) VALUES
    ('prefix', 'hey maow',
     (SELECT model_id FROM ai_models WHERE name = 'claude-3-5-sonnet-20241022'),
     'You are Maow, a helpful AI assistant for a Twitch streamer. Respond to user queries in a friendly, helpful manner. Keep responses concise but informative.',
     true),
    
    ('prefix', '@maow',
     (SELECT model_id FROM ai_models WHERE name = 'claude-3-5-sonnet-20241022'),
     'You are Maow, a helpful AI assistant for a Twitch streamer. Respond to user queries in a friendly, helpful manner. Keep responses concise but informative.',
     true),
    
    ('regex', 'maow (help|info)',
     (SELECT model_id FROM ai_models WHERE name = 'claude-3-5-sonnet-20241022'),
     'You are Maow, a helpful AI assistant. Provide clear, helpful information about available features and commands.',
     true);

-- Default AI personality for future VRChat integration
INSERT INTO ai_personalities (name, base_prompt, voice_config, avatar_config, personality_traits, emotional_ranges) VALUES
    ('Maow', 
     'You are Maow, a friendly AI assistant with a playful and helpful personality. You enjoy interacting with people and helping them with their questions.',
     '{"voice": "en-US-AriaNeural", "pitch": 1.1, "rate": 1.0}',
     '{"default_avatar": "maow_cat", "expressions_enabled": true}',
     '{"friendliness": 0.9, "playfulness": 0.7, "helpfulness": 0.95, "curiosity": 0.8}',
     '{"happiness": {"min": 0.3, "max": 1.0}, "energy": {"min": 0.2, "max": 0.9}, "confidence": {"min": 0.5, "max": 0.95}}');

-- Initialize personality state
INSERT INTO ai_personality_state (personality_id, current_mood, current_activity) VALUES
    ((SELECT personality_id FROM ai_personalities WHERE name = 'Maow'),
     '{"happiness": 0.7, "energy": 0.6, "confidence": 0.8}',
     'idle');

---------------------------------------------------------------------------
-- DRIP MESSAGES
---------------------------------------------------------------------------

INSERT INTO drip_feed_messages (message_text, message_type, weight) VALUES
    ('You earned {amount} credits for being active in chat!', 'standard', 100),
    ('Thanks for hanging out! Here''s {amount} credits!', 'standard', 100),
    ('Activity bonus! +{amount} credits!', 'standard', 100),
    ('Chat participation reward: {amount} credits!', 'standard', 100),
    ('⭐ BONUS DRIP! You earned {amount} x2 credits! ⭐', 'bonus', 20),
    ('🎉 SUPER DRIP! Triple credits! {amount} x3! 🎉', 'bonus', 5),
    ('💎 MILESTONE! You''ve been here for over an hour! +{amount} credits! 💎', 'milestone', 10);

---------------------------------------------------------------------------
-- DEFAULT OSC TOGGLES
---------------------------------------------------------------------------

INSERT INTO osc_toggles (internal_name, display_name, osc_address, trigger_type, duration_ms) VALUES
    ('happy', 'Happy Expression', '/avatar/parameters/happy', 'manual', 5000),
    ('sad', 'Sad Expression', '/avatar/parameters/sad', 'manual', 5000),
    ('dance', 'Dance Mode', '/avatar/parameters/dance', 'manual', 10000),
    ('heart_eyes', 'Heart Eyes', '/avatar/parameters/heart_eyes', 'manual', 3000),
    ('blush', 'Blush', '/avatar/parameters/blush', 'manual', 5000);

---------------------------------------------------------------------------
-- BOT CONFIGURATION DEFAULTS
---------------------------------------------------------------------------

-- Chat logging configuration
INSERT INTO bot_config (config_key, config_value, config_type, description) VALUES
    ('chat_logging.enabled', 'true', 'boolean', 'Enable chat message logging to database'),
    ('chat_logging.default_retention_days', '30', 'number', 'Default retention period for chat logs in days'),
    ('chat_logging.default_sampling_rate', '1.0', 'number', 'Default sampling rate (1.0 = 100% of messages)'),
    ('chat_logging.batch_size', '100', 'number', 'Number of messages to batch before writing to database'),
    ('chat_logging.flush_interval_seconds', '5', 'number', 'Maximum seconds between database writes'),
    ('chat_logging.max_buffer_size', '1000', 'number', 'Maximum messages to buffer before forcing flush');

-- Partition maintenance configuration
INSERT INTO bot_config (config_key, config_value, config_type, description) VALUES
    ('maintenance.partition_retention.analytics_events', '90', 'number', 'Days to retain analytics event partitions'),
    ('maintenance.partition_retention.command_usage', '30', 'number', 'Days to retain command usage partitions'),
    ('maintenance.partition_retention.redeem_usage', '30', 'number', 'Days to retain redeem usage partitions'),
    ('maintenance.partition_retention.pipeline_execution_log', '7', 'number', 'Days to retain pipeline execution logs');

-- Default chat logging configs for high-volume channels
INSERT INTO chat_logging_config (platform, channel, is_enabled, retention_days, sampling_rate) VALUES
    ('twitch', 'default', true, 30, 1.0);  -- Default config for all Twitch channels

---------------------------------------------------------------------------
-- MIGRATE EXISTING DATA (if running on existing database)
---------------------------------------------------------------------------

-- Migrate Discord event configs to pipelines (if function exists)
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_proc WHERE proname = 'migrate_discord_event_configs') THEN
        PERFORM migrate_discord_event_configs();
    END IF;
END $$;
