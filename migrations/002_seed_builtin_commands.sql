
INSERT INTO commands (
    platform,
    command_name,
    min_role,
    is_active,
    created_at,
    updated_at,
    cooldown_seconds,
    cooldown_warnonce,
    stream_online_only,
    stream_offline_only
)
VALUES
    ('twitch-irc', 'ping',      'everyone', true, now(), now(), 0, false, false, false),
    ('twitch-irc', 'followage', 'everyone', true, now(), now(), 0, false, false, false),
    ('twitch-irc', 'world',     'everyone', true, now(), now(), 0, false, false, false),
    ('twitch-irc', 'instance',  'everyone', true, now(), now(), 0, false, false, false),
    ('twitch-irc', 'vrchat',    'mod',      true, now(), now(), 0, false, false, false)
    ('twitch-irc', 'vanish',    'everyone', true, now(), now(), 0, false, false, false)
    ON CONFLICT (platform, LOWER(command_name)) DO NOTHING;
