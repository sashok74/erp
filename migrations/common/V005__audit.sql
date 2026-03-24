-- V005: Structured audit log.
-- Записывается Pipeline'ом после успешного commit'а команды.
CREATE TABLE IF NOT EXISTS common.audit_log (
    id              BIGSERIAL PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    user_id         UUID NOT NULL,
    correlation_id  UUID NOT NULL,
    action          TEXT NOT NULL,          -- "warehouse::receive_goods" (:: convention!)
    entity_type     TEXT,
    entity_id       UUID,
    old_state       JSONB,
    new_state       JSONB,
    metadata        JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_audit_tenant_time
    ON common.audit_log (tenant_id, created_at DESC);

-- RLS: tenant isolation.
ALTER TABLE common.audit_log ENABLE ROW LEVEL SECURITY;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies WHERE tablename = 'audit_log' AND policyname = 'tenant_iso'
    ) THEN
        CREATE POLICY tenant_iso ON common.audit_log
            USING (tenant_id = common.current_tenant_id());
    END IF;
END $$;
