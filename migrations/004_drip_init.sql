-- maowbot-core/migrations/000X_drip_init.sql
-- Drip Commands Migration: New tables for OSC avatar outfits and prop/timer functionality.
-- Completely rewritten for UUID-based primary keys.

-- Enable the uuid-ossp extension (for uuid_generate_v4, if on PostgreSQL).
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

---------------------------------------------------------------------------
-- Drop Existing Drip Tables (if they exist)
---------------------------------------------------------------------------
DROP TABLE IF EXISTS drip_avatar_params CASCADE;
DROP TABLE IF EXISTS drip_avatar_props CASCADE;
DROP TABLE IF EXISTS drip_prop_timers CASCADE;
DROP TABLE IF EXISTS drip_prop_params CASCADE;
DROP TABLE IF EXISTS drip_props CASCADE;
DROP TABLE IF EXISTS drip_fit_params CASCADE;
DROP TABLE IF EXISTS drip_fits CASCADE;
DROP TABLE IF EXISTS drip_avatar_prefix_rules CASCADE;
DROP TABLE IF EXISTS drip_avatars CASCADE;

---------------------------------------------------------------------------
-- drip_avatars
-- Stores each discovered avatar from VRChat, linked to our local user.
---------------------------------------------------------------------------
CREATE TABLE drip_avatars (
    drip_avatar_id      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id             UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
    vrchat_avatar_id    TEXT NOT NULL,
    vrchat_avatar_name  TEXT NOT NULL,
    local_name          TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- drip_avatar_prefix_rules
-- Stores "ignore" or "strip" prefix rules for an avatar.
---------------------------------------------------------------------------
CREATE TABLE drip_avatar_prefix_rules (
    drip_avatar_prefix_rule_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    drip_avatar_id             UUID NOT NULL REFERENCES drip_avatars(drip_avatar_id) ON DELETE CASCADE,
    rule_type                TEXT NOT NULL CHECK (rule_type IN ('ignore','strip')),
    prefix                   TEXT NOT NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- drip_fits
-- Represents each outfit ("fit") defined for an avatar.
---------------------------------------------------------------------------
CREATE TABLE drip_fits (
    drip_fit_id    UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    drip_avatar_id UUID NOT NULL REFERENCES drip_avatars(drip_avatar_id) ON DELETE CASCADE,
    fit_name       TEXT NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- drip_fit_params
-- Holds individual parameter name/value pairs for each outfit.
---------------------------------------------------------------------------
CREATE TABLE drip_fit_params (
    drip_fit_param_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    drip_fit_id       UUID NOT NULL REFERENCES drip_fits(drip_fit_id) ON DELETE CASCADE,
    param_name        TEXT NOT NULL,
    param_value       TEXT NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- drip_props
-- Represents a reusable "prop" that can be applied to avatars.
---------------------------------------------------------------------------
CREATE TABLE drip_props (
    drip_prop_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    prop_name    TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- drip_prop_params
-- Defines static key/value parameters that a prop applies when activated.
---------------------------------------------------------------------------
CREATE TABLE drip_prop_params (
    drip_prop_param_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    drip_prop_id       UUID NOT NULL REFERENCES drip_props(drip_prop_id) ON DELETE CASCADE,
    param_name         TEXT NOT NULL,
    param_value        TEXT NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- drip_prop_timers
-- Stores timer or sequencing data for a prop as structured JSONB.
---------------------------------------------------------------------------
CREATE TABLE drip_prop_timers (
    drip_prop_timer_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    drip_prop_id       UUID NOT NULL REFERENCES drip_props(drip_prop_id) ON DELETE CASCADE,
    timer_data         JSONB NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);


---------------------------------------------------------------------------
-- drip_avatar_props (Optional Bridge Table)
-- Associates a given prop with a specific avatar. Useful for per-avatar overrides.
---------------------------------------------------------------------------
CREATE TABLE drip_avatar_props (
    drip_avatar_prop_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    drip_avatar_id      UUID NOT NULL REFERENCES drip_avatars(drip_avatar_id) ON DELETE CASCADE,
    drip_prop_id        UUID NOT NULL REFERENCES drip_props(drip_prop_id) ON DELETE CASCADE,
    override_params     JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------------------
-- drip_avatar_params (Optional Discovery Log)
-- Records the parameters discovered from the VRChat config for an avatar.
---------------------------------------------------------------------------
CREATE TABLE drip_avatar_params (
    drip_avatar_param_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    drip_avatar_id       UUID NOT NULL REFERENCES drip_avatars(drip_avatar_id) ON DELETE CASCADE,
    param_name           TEXT NOT NULL,
    param_type           TEXT NOT NULL,
    discovered_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);