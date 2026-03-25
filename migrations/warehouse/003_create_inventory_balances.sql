CREATE TABLE IF NOT EXISTS warehouse.inventory_balances (
    tenant_id        UUID NOT NULL,
    item_id          UUID NOT NULL,
    sku              TEXT NOT NULL,
    balance          NUMERIC(18,4) NOT NULL DEFAULT 0,
    last_movement_id UUID,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, item_id)
);

CREATE INDEX IF NOT EXISTS idx_balances_sku ON warehouse.inventory_balances (tenant_id, sku);
