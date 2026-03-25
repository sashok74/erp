CREATE TABLE IF NOT EXISTS warehouse.product_projections (
    tenant_id       UUID NOT NULL,
    product_id      UUID NOT NULL,
    sku             TEXT NOT NULL,
    name            TEXT NOT NULL,
    category        TEXT NOT NULL DEFAULT '',
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_product_proj_sku
    ON warehouse.product_projections (tenant_id, sku);

ALTER TABLE warehouse.product_projections ENABLE ROW LEVEL SECURITY;
DO $$ BEGIN
    CREATE POLICY tenant_iso ON warehouse.product_projections
        USING (tenant_id = common.current_tenant_id());
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
