-- migrations/008_ai_redeems.sql
--
-- Add is_user_input_required field to the redeems table
-- and seed the AI redeems

-- Add is_user_input_required field to redeems table
ALTER TABLE redeems ADD COLUMN IF NOT EXISTS is_user_input_required BOOLEAN NOT NULL DEFAULT FALSE;

-- Update existing redeems to set appropriate values
UPDATE redeems SET is_user_input_required = FALSE WHERE command_name IN ('cute', 'pillo');

-- Add new AI redeems
INSERT INTO redeems (
    redeem_id,
    platform,
    reward_id,
    reward_name,
    cost,
    is_input_required,
    redeem_prompt_text,
    is_active,
    dynamic_pricing,
    active_offline,
    is_managed,
    created_at,
    updated_at,
    plugin_name,
    command_name
)

VALUES (
   uuid_generate_v4(),
   'twitch-eventsub',
   '',
   'ask synapsy',
   2001,
   true,
   'get a quick ai answer',
   true,
   false,
   true,
   true,
   now(),
   now(),
   'builtin',
   'askai'
),
(
   uuid_generate_v4(),
   'twitch-eventsub',
   '',
   'synapsy—web search',
   2002,
   true,
   'get a quick ai answer—with web search!',
   true,
   false,
   true,
   true,
   now(),
   now(),
   'builtin',
   'askai_search'
),
(
   uuid_generate_v4(),
   'twitch-eventsub',
   '',
   'maow',
   404,
   true,
   'real answers!',
   true,
   false,
   true,
   true,
   now(),
   now(),
   'builtin',
   'askmao'
)
