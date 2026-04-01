-- Force RLS: table owner также подчиняется RLS-политикам.
-- Архитектурное ревью 2026-04-01: P0 finding.

ALTER TABLE warehouse.inventory_items FORCE ROW LEVEL SECURITY;
ALTER TABLE warehouse.stock_movements FORCE ROW LEVEL SECURITY;
ALTER TABLE warehouse.inventory_balances FORCE ROW LEVEL SECURITY;
ALTER TABLE warehouse.product_projections FORCE ROW LEVEL SECURITY;
