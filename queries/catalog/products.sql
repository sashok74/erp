--! create_product
INSERT INTO catalog.products (tenant_id, id, sku, name, category, unit)
VALUES (:tenant_id, :id, :sku, :name, :category, :unit);

--! find_by_sku
SELECT id, sku, name, category, unit
FROM catalog.products
WHERE tenant_id = :tenant_id AND sku = :sku;

--! find_by_id
SELECT id, sku, name, category, unit
FROM catalog.products
WHERE tenant_id = :tenant_id AND id = :id;
