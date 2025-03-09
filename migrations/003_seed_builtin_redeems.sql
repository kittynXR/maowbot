-- migrations/003_seed_builtin_redeems.sql
--
-- We seed our demonstration "cute" redeem as a built-in, managed redeem.
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
    command_name
)
VALUES (
           uuid_generate_v4(),
           'twitch-eventsub',       -- Our local platform identifier for EventSub
           '',                      -- reward_id is empty for built-in seeds
           'cute',                  -- display name in DB
           50,                      -- cost
           true,                    -- is_active
           false,                   -- dynamic_pricing
           true,                    -- active_offline
           true,                    -- is_managed
           now(),
           now(),
           'builtin',               -- plugin_name => "builtin"
           'cute'                   -- command_name => "cute"
       )
    ON CONFLICT (platform, reward_id) DO NOTHING;