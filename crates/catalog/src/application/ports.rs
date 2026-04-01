//! Порты persistence — repo struct и DTO для доступа к данным.
//!
//! `ProductRepo` держит tenant-scoped DB context (`&Client`, `TenantId`).
//! Методы реализованы в `infrastructure::repos` (split impl pattern).

use kernel::types::TenantId;
use uuid::Uuid;

/// Строка товара для query handler.
#[derive(Debug)]
pub struct ProductRow {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

/// Tenant-scoped repository для товаров каталога.
pub struct ProductRepo<'a> {
    pub(crate) client: &'a deadpool_postgres::Client,
    pub(crate) tenant_id: TenantId,
}

impl<'a> ProductRepo<'a> {
    /// Создать tenant-scoped repo.
    #[must_use]
    pub fn new(client: &'a deadpool_postgres::Client, tenant_id: TenantId) -> Self {
        Self { client, tenant_id }
    }
}
