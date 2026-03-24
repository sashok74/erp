-- V008: Domain history — снимки состояния сущности до/после каждой команды.
-- Связь с audit_log через correlation_id + causation_id.
-- Engineering invariant #3: все write-операции — с audit log + domain history.
CREATE TABLE IF NOT EXISTS common.domain_history (
    id              BIGSERIAL PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    entity_type     TEXT NOT NULL,
    entity_id       UUID NOT NULL,
    event_type      TEXT NOT NULL,          -- "erp.warehouse.goods_received.v1"
    old_state       JSONB,
    new_state       JSONB,
    correlation_id  UUID NOT NULL,
    causation_id    UUID NOT NULL,
    user_id         UUID NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_history_entity
    ON common.domain_history (tenant_id, entity_type, entity_id, created_at DESC);

-- RLS: tenant isolation.
ALTER TABLE common.domain_history ENABLE ROW LEVEL SECURITY;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies WHERE tablename = 'domain_history' AND policyname = 'tenant_iso'
    ) THEN
        CREATE POLICY tenant_iso ON common.domain_history
            USING (tenant_id = common.current_tenant_id());
    END IF;
END $$;
