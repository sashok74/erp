-- V012: FORCE ROW LEVEL SECURITY на infrastructure-таблицах.
-- Без FORCE владелец таблиц (erp_admin) обходит RLS-политики.
-- Архитектурное ревью 2026-04-01: P0 finding.
--
-- outbox и dead_letters: НЕ FORCE, т.к. OutboxRelay — системный процесс,
-- читающий события всех tenants для публикации. RLS policy остаётся
-- (для tenant-scoped чтения из API), но owner (relay) может обходить.

ALTER TABLE common.audit_log FORCE ROW LEVEL SECURITY;
ALTER TABLE common.sequences FORCE ROW LEVEL SECURITY;
ALTER TABLE common.domain_history FORCE ROW LEVEL SECURITY;
