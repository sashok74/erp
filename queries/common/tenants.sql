--! get_tenant
SELECT id, name, slug, is_active, created_at, updated_at
FROM common.tenants
WHERE id = :id;

--! create_tenant
INSERT INTO common.tenants (id, name, slug)
VALUES (:id, :name, :slug)
RETURNING id, name, slug, is_active, created_at, updated_at;
