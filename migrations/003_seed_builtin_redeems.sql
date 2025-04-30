-- migrations/003_seed_builtin_redeems.sql
--
-- We seed our demonstration "cute" redeem as a built-in, managed redeem.

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
    'cute',
    50,
    true,
    'chat cute',
    true,
    false,
    true,
    true,
    now(),
    now(),
    'builtin',
    'cute'
),
(
    uuid_generate_v4(),
    'twitch-eventsub',
    '',
    'comfi pillo',
    25,
    false,
    'toss a pillo at stremer (might bonk)',
    false,
    false,
    true,
    true,
    now(),
    now(),
    'builtin',
    'pillo'
)

ON CONFLICT (plugin_name, command_name) DO NOTHING;
