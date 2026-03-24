-- V003: Функция для извлечения tenant_id из session variable.
-- Используется в RLS-политиках всех таблиц.
-- SET LOCAL app.tenant_id = '<uuid>' устанавливается в PgUnitOfWork::begin().
CREATE OR REPLACE FUNCTION common.current_tenant_id()
RETURNS UUID AS $$
    SELECT NULLIF(current_setting('app.tenant_id', true), '')::UUID;
$$ LANGUAGE sql STABLE;
