ALTER TABLE warehouse.inventory_items ENABLE ROW LEVEL SECURITY;
DO $$ BEGIN
    CREATE POLICY tenant_iso ON warehouse.inventory_items
        USING (tenant_id = common.current_tenant_id());
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

ALTER TABLE warehouse.stock_movements ENABLE ROW LEVEL SECURITY;
DO $$ BEGIN
    CREATE POLICY tenant_iso ON warehouse.stock_movements
        USING (tenant_id = common.current_tenant_id());
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

ALTER TABLE warehouse.inventory_balances ENABLE ROW LEVEL SECURITY;
DO $$ BEGIN
    CREATE POLICY tenant_iso ON warehouse.inventory_balances
        USING (tenant_id = common.current_tenant_id());
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
