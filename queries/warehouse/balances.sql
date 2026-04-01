-- queries/warehouse/balances.sql
-- Запросы к warehouse.inventory_balances.
-- Source of truth для clorinde-gen/src/warehouse/balances.rs.

--! upsert_balance
--@ repo: balances
--@ kind: exec
--@ dec: balance
INSERT INTO warehouse.inventory_balances
    (tenant_id, item_id, sku, balance, last_movement_id, updated_at)
VALUES (:tenant_id, :item_id, :sku, :balance::TEXT::NUMERIC, :last_movement_id, now())
ON CONFLICT (tenant_id, item_id) DO UPDATE SET
    balance = EXCLUDED.balance,
    last_movement_id = EXCLUDED.last_movement_id,
    updated_at = now();

--! get_balance : (balance)
--@ repo: balances
--@ kind: opt
--@ dto: BalanceRow
--@ dec: balance
SELECT item_id, sku, balance::TEXT
FROM warehouse.inventory_balances
WHERE tenant_id = :tenant_id AND sku = :sku;
