-- queries/warehouse/inventory.sql
-- Запросы к warehouse.inventory_items и warehouse.stock_movements.
-- Source of truth для clorinde-gen/src/warehouse/inventory.rs.

--! find_item_by_sku : (balance)
--@ repo: inventory
--@ kind: opt
--@ dto: InventoryItemLookupRow
--@ dec: balance
SELECT i.id, COALESCE(b.balance, 0)::TEXT AS balance
FROM warehouse.inventory_items i
LEFT JOIN warehouse.inventory_balances b
    ON b.tenant_id = i.tenant_id AND b.item_id = i.id
WHERE i.tenant_id = :tenant_id AND i.sku = :sku;

--! create_item
--@ repo: inventory
--@ kind: exec
INSERT INTO warehouse.inventory_items (tenant_id, id, sku)
VALUES (:tenant_id, :id, :sku);

--! insert_movement
--@ repo: inventory
--@ kind: exec
--@ input: NewStockMovement
--@ dec: quantity,balance_after
INSERT INTO warehouse.stock_movements
    (tenant_id, id, item_id, event_type, quantity, balance_after,
     doc_number, correlation_id, user_id)
VALUES
    (:tenant_id, :id, :item_id, :event_type,
     :quantity::TEXT::NUMERIC, :balance_after::TEXT::NUMERIC,
     :doc_number, :correlation_id, :user_id);
