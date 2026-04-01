--! find_by_sku
--@ repo: items
--@ kind: opt
--@ dto: ItemRow
SELECT id, sku, name
FROM test.items
WHERE tenant_id = :tenant_id AND sku = :sku;

--! create_item
--@ repo: items
--@ kind: exec
INSERT INTO test.items (tenant_id, id, sku, name)
VALUES (:tenant_id, :id, :sku, :name);
