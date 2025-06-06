-- Create autostart table for platform connection autostart configuration
CREATE TABLE IF NOT EXISTS autostart (
    id SERIAL PRIMARY KEY,
    platform VARCHAR(50) NOT NULL,
    account_name VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Ensure we don't have duplicate entries for the same platform/account
    UNIQUE(platform, account_name)
);

-- Create index for faster lookups
CREATE INDEX idx_autostart_enabled ON autostart(enabled) WHERE enabled = true;

-- Add update trigger for updated_at
CREATE OR REPLACE FUNCTION update_autostart_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER autostart_updated_at
    BEFORE UPDATE ON autostart
    FOR EACH ROW
    EXECUTE FUNCTION update_autostart_updated_at();

-- Migrate existing autostart data from bot_config if it exists
DO $$
DECLARE
    autostart_json TEXT;
    config_data JSONB;
    account_entry RECORD;
BEGIN
    -- Get the current autostart config
    SELECT config_value INTO autostart_json
    FROM bot_config
    WHERE config_key = 'autostart'
    LIMIT 1;
    
    IF autostart_json IS NOT NULL THEN
        config_data := autostart_json::JSONB;
        
        -- Check if it's the new format with 'accounts' array
        IF config_data ? 'accounts' THEN
            -- Process new format: {"accounts": [["platform", "account"]]}
            FOR account_entry IN 
                SELECT value->>0 as platform, value->>1 as account 
                FROM jsonb_array_elements(config_data->'accounts')
            LOOP
                INSERT INTO autostart (platform, account_name, enabled)
                VALUES (account_entry.platform, account_entry.account, true)
                ON CONFLICT (platform, account_name) DO NOTHING;
            END LOOP;
        ELSE
            -- Process old format: {"discord": ["account1"], "twitch-irc": ["account2"]}
            -- Discord accounts
            IF config_data ? 'discord' THEN
                FOR account_entry IN 
                    SELECT value::text as account 
                    FROM jsonb_array_elements_text(config_data->'discord')
                LOOP
                    INSERT INTO autostart (platform, account_name, enabled)
                    VALUES ('discord', account_entry.account, true)
                    ON CONFLICT (platform, account_name) DO NOTHING;
                END LOOP;
            END IF;
            
            -- Twitch IRC accounts
            IF config_data ? 'twitch_irc' THEN
                FOR account_entry IN 
                    SELECT value::text as account 
                    FROM jsonb_array_elements_text(config_data->'twitch_irc')
                LOOP
                    INSERT INTO autostart (platform, account_name, enabled)
                    VALUES ('twitch-irc', account_entry.account, true)
                    ON CONFLICT (platform, account_name) DO NOTHING;
                END LOOP;
            END IF;
            
            -- Twitch EventSub accounts
            IF config_data ? 'twitch_eventsub' THEN
                FOR account_entry IN 
                    SELECT value::text as account 
                    FROM jsonb_array_elements_text(config_data->'twitch_eventsub')
                LOOP
                    INSERT INTO autostart (platform, account_name, enabled)
                    VALUES ('twitch-eventsub', account_entry.account, true)
                    ON CONFLICT (platform, account_name) DO NOTHING;
                END LOOP;
            END IF;
            
            -- VRChat accounts
            IF config_data ? 'vrchat' THEN
                FOR account_entry IN 
                    SELECT value::text as account 
                    FROM jsonb_array_elements_text(config_data->'vrchat')
                LOOP
                    INSERT INTO autostart (platform, account_name, enabled)
                    VALUES ('vrchat', account_entry.account, true)
                    ON CONFLICT (platform, account_name) DO NOTHING;
                END LOOP;
            END IF;
        END IF;
        
        -- Optionally remove the old autostart config from bot_config
        -- DELETE FROM bot_config WHERE config_key = 'autostart';
    END IF;
END $$;