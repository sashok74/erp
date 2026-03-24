-- V006: Gap-free sequence generator per tenant.
-- Используется для номеров документов: "WH-000042", "INV-000001".
-- SELECT FOR UPDATE + increment в одной TX гарантирует отсутствие пропусков.
CREATE TABLE IF NOT EXISTS common.sequences (
    tenant_id   UUID NOT NULL,
    seq_name    TEXT NOT NULL,          -- "warehouse::receipt"
    prefix      TEXT NOT NULL DEFAULT '',
    next_value  BIGINT NOT NULL DEFAULT 1,
    PRIMARY KEY (tenant_id, seq_name)
);

-- RLS: tenant isolation.
ALTER TABLE common.sequences ENABLE ROW LEVEL SECURITY;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies WHERE tablename = 'sequences' AND policyname = 'tenant_iso'
    ) THEN
        CREATE POLICY tenant_iso ON common.sequences
            USING (tenant_id = common.current_tenant_id());
    END IF;
END $$;
