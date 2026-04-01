-- Force RLS: table owner также подчиняется RLS-политикам.
-- Архитектурное ревью 2026-04-01: P0 finding.

ALTER TABLE catalog.products FORCE ROW LEVEL SECURITY;
