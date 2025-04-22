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
    is_active,
    dynamic_pricing,
    active_offline,
    is_managed,
    created_at,
    updated_at,
    plugin_name,
    command_name,
    is_user_input_required
)
VALUES (
    uuid_generate_v4(),
    'twitch-eventsub',
    '',
    'Ask AI',
    100,
    true,
    false,
    true,
    true,
    now(),
    now(),
    'builtin',
    'askai',
    true
),
(
    uuid_generate_v4(),
    'twitch-eventsub',
    '',
    'Ask Maowbot',
    100,
    true,
    false,
    true,
    true,
    now(),
    now(),
    'builtin',
    'askmao',
    true
),
(
    uuid_generate_v4(),
    'twitch-eventsub',
    '',
    'Ask AI with Search',
    200,
    true,
    false,
    true,
    true,
    now(),
    now(),
    'builtin',
    'askai_search',
    true
)
ON CONFLICT (plugin_name, command_name) DO NOTHING;