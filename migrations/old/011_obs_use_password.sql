-- Add use_password column to obs_instances table
ALTER TABLE obs_instances 
ADD COLUMN IF NOT EXISTS use_password BOOLEAN NOT NULL DEFAULT true;

-- Ensure system users exist for OBS instances (in case they're missing)
-- First check if they exist, then insert if needed
INSERT INTO users (user_id, global_username, created_at, last_seen, is_active)
SELECT uuid_generate_v4(), 'obs-1', NOW(), NOW(), true
WHERE NOT EXISTS (SELECT 1 FROM users WHERE global_username = 'obs-1');

INSERT INTO users (user_id, global_username, created_at, last_seen, is_active)
SELECT uuid_generate_v4(), 'obs-2', NOW(), NOW(), true
WHERE NOT EXISTS (SELECT 1 FROM users WHERE global_username = 'obs-2');

-- Ensure credentials exist for OBS instances
-- Note: OBS doesn't use traditional credentials, but we need placeholder entries
-- for the platform manager to recognize these accounts
DO $$
DECLARE
    obs1_user_id UUID;
    obs2_user_id UUID;
BEGIN
    -- Get user IDs
    SELECT user_id INTO obs1_user_id FROM users WHERE global_username = 'obs-1';
    SELECT user_id INTO obs2_user_id FROM users WHERE global_username = 'obs-2';
    
    -- Create placeholder credentials if they don't exist
    -- First check and insert for obs-1
    IF obs1_user_id IS NOT NULL THEN
        INSERT INTO platform_credentials (credential_id, user_id, platform, credential_type, user_name, primary_token, expires_at, created_at, updated_at, is_bot)
        SELECT uuid_generate_v4(), obs1_user_id, 'obs', 'instance', 'obs-1', 'obs-instance-1', NOW() + INTERVAL '100 years', NOW(), NOW(), false
        WHERE NOT EXISTS (
            SELECT 1 FROM platform_credentials 
            WHERE user_id = obs1_user_id AND platform = 'obs'
        );
    END IF;
    
    -- Then check and insert for obs-2
    IF obs2_user_id IS NOT NULL THEN
        INSERT INTO platform_credentials (credential_id, user_id, platform, credential_type, user_name, primary_token, expires_at, created_at, updated_at, is_bot)
        SELECT uuid_generate_v4(), obs2_user_id, 'obs', 'instance', 'obs-2', 'obs-instance-2', NOW() + INTERVAL '100 years', NOW(), NOW(), false
        WHERE NOT EXISTS (
            SELECT 1 FROM platform_credentials 
            WHERE user_id = obs2_user_id AND platform = 'obs'
        );
    END IF;
END $$;