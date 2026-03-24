-- V002: Реестр tenant'ов.
-- tenants — единственная таблица БЕЗ RLS (нет tenant_id — она сама является справочником tenant'ов).
CREATE TABLE IF NOT EXISTS common.tenants (
    id          UUID PRIMARY KEY,
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL UNIQUE,
    is_active   BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
