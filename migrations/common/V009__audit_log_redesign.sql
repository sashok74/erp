-- V009: Redesign audit_log — entity snapshots moved to domain_history.
-- Rename `action` → `command_name`, add `result` + `causation_id`,
-- drop columns now covered by domain_history.

-- Rename action → command_name.
ALTER TABLE common.audit_log
    RENAME COLUMN action TO command_name;

-- Add new columns.
ALTER TABLE common.audit_log
    ADD COLUMN IF NOT EXISTS result       JSONB NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS causation_id UUID;

-- Drop columns migrated to domain_history.
ALTER TABLE common.audit_log
    DROP COLUMN IF EXISTS entity_type,
    DROP COLUMN IF EXISTS entity_id,
    DROP COLUMN IF EXISTS old_state,
    DROP COLUMN IF EXISTS new_state,
    DROP COLUMN IF EXISTS metadata;
