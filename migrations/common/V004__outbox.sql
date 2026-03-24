-- V004: Transactional outbox для at-least-once event delivery.
-- События записываются в outbox В ТОЙ ЖЕ транзакции, что и бизнес-данные.
-- Outbox Relay (Layer 3b) публикует их асинхронно.
CREATE TABLE IF NOT EXISTS common.outbox (
    id              BIGSERIAL PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    event_id        UUID NOT NULL UNIQUE,
    event_type      TEXT NOT NULL,          -- "erp.warehouse.goods_shipped.v1" (CloudEvents convention)
    source          TEXT NOT NULL,          -- "warehouse"
    payload         JSONB NOT NULL,
    correlation_id  UUID NOT NULL,
    causation_id    UUID NOT NULL,
    user_id         UUID NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    published       BOOLEAN NOT NULL DEFAULT false,
    published_at    TIMESTAMPTZ,
    retry_count     INT NOT NULL DEFAULT 0
);

-- Индекс для Outbox Relay: быстрый поиск неопубликованных событий.
CREATE INDEX IF NOT EXISTS idx_outbox_unpublished
    ON common.outbox (id) WHERE published = false;

-- Индекс для запросов по tenant + времени.
CREATE INDEX IF NOT EXISTS idx_outbox_tenant_time
    ON common.outbox (tenant_id, created_at DESC);

-- RLS: tenant isolation.
ALTER TABLE common.outbox ENABLE ROW LEVEL SECURITY;

-- Policy: idempotent через DO block.
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies WHERE tablename = 'outbox' AND policyname = 'tenant_iso'
    ) THEN
        CREATE POLICY tenant_iso ON common.outbox
            USING (tenant_id = common.current_tenant_id());
    END IF;
END $$;
