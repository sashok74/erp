-- V011: Dead Letter Queue для событий, превысивших лимит retry в outbox relay.
-- Вместо молчаливого пропуска — перенос в dead_letters с диагностической информацией.
CREATE TABLE IF NOT EXISTS common.dead_letters (
    id                 BIGSERIAL PRIMARY KEY,
    event_id           UUID NOT NULL UNIQUE,
    event_type         TEXT NOT NULL,
    source             TEXT NOT NULL,
    tenant_id          UUID NOT NULL,
    payload            JSONB NOT NULL,
    correlation_id     UUID NOT NULL,
    causation_id       UUID NOT NULL,
    user_id            UUID NOT NULL,
    original_created_at TIMESTAMPTZ NOT NULL,
    failed_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    retry_count        INT NOT NULL,
    last_error         TEXT NOT NULL DEFAULT ''
);

-- Индекс для операторской диагностики по tenant + времени.
CREATE INDEX IF NOT EXISTS idx_dead_letters_tenant_time
    ON common.dead_letters (tenant_id, failed_at DESC);

-- RLS: tenant isolation.
ALTER TABLE common.dead_letters ENABLE ROW LEVEL SECURITY;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies WHERE tablename = 'dead_letters' AND policyname = 'tenant_iso'
    ) THEN
        CREATE POLICY tenant_iso ON common.dead_letters
            USING (tenant_id = common.current_tenant_id());
    END IF;
END $$;
