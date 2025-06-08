-- Create obs_instances table for OBS WebSocket connection configuration
CREATE TABLE IF NOT EXISTS obs_instances (
    instance_id SERIAL PRIMARY KEY,
    instance_number INT NOT NULL UNIQUE CHECK (instance_number > 0),
    host VARCHAR(255) NOT NULL,
    port INT NOT NULL DEFAULT 4455 CHECK (port > 0 AND port <= 65535),
    use_ssl BOOLEAN NOT NULL DEFAULT false,
    password_encrypted TEXT,
    is_connected BOOLEAN NOT NULL DEFAULT false,
    last_connected_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index for faster lookups
CREATE INDEX idx_obs_instances_instance_number ON obs_instances(instance_number);

-- Add update trigger for updated_at
CREATE OR REPLACE FUNCTION update_obs_instances_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER obs_instances_updated_at
    BEFORE UPDATE ON obs_instances
    FOR EACH ROW
    EXECUTE FUNCTION update_obs_instances_updated_at();

-- Seed with default OBS instances
INSERT INTO obs_instances (instance_number, host, port, use_ssl, password_encrypted)
VALUES 
    (1, '127.0.0.1', 4455, false, NULL),
    (2, '10.11.11.111', 4455, false, NULL)
ON CONFLICT (instance_number) DO NOTHING;

-- Create system users for OBS instances
-- We need user entries for the platform manager to work properly
DO $$
DECLARE
    user1_id UUID;
    user2_id UUID;
BEGIN
    -- Create user for OBS instance 1
    INSERT INTO users (user_id, global_username, created_at, last_seen, is_active)
    VALUES (uuid_generate_v4(), 'obs-1', NOW(), NOW(), true)
    ON CONFLICT DO NOTHING
    RETURNING user_id INTO user1_id;
    
    -- Create user for OBS instance 2
    INSERT INTO users (user_id, global_username, created_at, last_seen, is_active)
    VALUES (uuid_generate_v4(), 'obs-2', NOW(), NOW(), true)
    ON CONFLICT DO NOTHING
    RETURNING user_id INTO user2_id;
END $$;

-- Add OBS-specific entries to autostart table for compatibility
-- These will use format 'obs-<instance_number>' as the account_name
INSERT INTO autostart (platform, account_name, enabled)
VALUES 
    ('obs', 'obs-1', false),
    ('obs', 'obs-2', false)
ON CONFLICT (platform, account_name) DO NOTHING;