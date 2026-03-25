ALTER TABLE catalog.products ENABLE ROW LEVEL SECURITY;
DO $$ BEGIN
    CREATE POLICY tenant_iso ON catalog.products
        USING (tenant_id = common.current_tenant_id());
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
