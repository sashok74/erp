--! upsert_product_projection
--@ repo: projections
--@ kind: exec
INSERT INTO warehouse.product_projections
    (tenant_id, product_id, sku, name, category, updated_at)
VALUES (:tenant_id, :product_id, :sku, :name, :category, now())
ON CONFLICT (tenant_id, product_id) DO UPDATE SET
    sku = EXCLUDED.sku,
    name = EXCLUDED.name,
    category = EXCLUDED.category,
    updated_at = now();

--! get_projection_by_sku
--@ repo: projections
--@ kind: opt
--@ dto: ProductProjectionRow
SELECT product_id, name, category
FROM warehouse.product_projections
WHERE tenant_id = :tenant_id AND sku = :sku;
