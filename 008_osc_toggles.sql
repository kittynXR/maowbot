-- OSC Toggle Configuration Tables

-- Store OSC trigger configurations that map redeems to avatar parameters
CREATE TABLE IF NOT EXISTS osc_triggers (
    id SERIAL PRIMARY KEY,
    redeem_id UUID NOT NULL REFERENCES redeems(redeem_id) ON DELETE CASCADE,
    parameter_name VARCHAR(255) NOT NULL,
    parameter_type VARCHAR(50) NOT NULL CHECK (parameter_type IN ('bool', 'int', 'float')),
    on_value TEXT NOT NULL, -- JSON encoded value
    off_value TEXT NOT NULL, -- JSON encoded value
    duration_seconds INTEGER, -- NULL means permanent toggle
    cooldown_seconds INTEGER DEFAULT 0,
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(redeem_id, parameter_name)
);

-- Store active toggle states
CREATE TABLE IF NOT EXISTS osc_toggle_states (
    id SERIAL PRIMARY KEY,
    trigger_id INTEGER NOT NULL REFERENCES osc_triggers(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    avatar_id VARCHAR(255), -- VRChat avatar ID
    activated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP, -- When the toggle should turn off
    is_active BOOLEAN DEFAULT true,
    UNIQUE(trigger_id, user_id, is_active) -- Only one active toggle per trigger per user
);

-- Store per-avatar toggle configurations (future feature)
CREATE TABLE IF NOT EXISTS osc_avatar_configs (
    id SERIAL PRIMARY KEY,
    avatar_id VARCHAR(255) NOT NULL,
    avatar_name VARCHAR(255),
    parameter_mappings JSONB, -- Store custom parameter mappings per avatar
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(avatar_id)
);

-- Insert default OSC trigger configurations for existing redeems
INSERT INTO osc_triggers (redeem_id, parameter_name, parameter_type, on_value, off_value, duration_seconds)
SELECT redeem_id, 'CatTrap', 'bool', 'true', 'false', 30
FROM redeems WHERE reward_name = 'cat_trap'
ON CONFLICT (redeem_id, parameter_name) DO NOTHING;

INSERT INTO osc_triggers (redeem_id, parameter_name, parameter_type, on_value, off_value, duration_seconds)
SELECT redeem_id, 'Pillo', 'bool', 'true', 'false', 60
FROM redeems WHERE reward_name = 'pillo'
ON CONFLICT (redeem_id, parameter_name) DO NOTHING;