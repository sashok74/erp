CREATE TABLE IF NOT EXISTS warehouse.stock_movements (
    tenant_id       UUID NOT NULL,
    id              UUID NOT NULL,
    item_id         UUID NOT NULL,
    event_type      TEXT NOT NULL,
    quantity        NUMERIC(18,4) NOT NULL,
    balance_after   NUMERIC(18,4) NOT NULL,
    doc_number      TEXT,
    correlation_id  UUID NOT NULL,
    user_id         UUID NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, id)
);

CREATE INDEX IF NOT EXISTS idx_movements_item ON warehouse.stock_movements (tenant_id, item_id, created_at DESC);
