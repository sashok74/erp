//! Порты persistence — repo struct и DTO для доступа к данным.
//!
//! `InventoryRepo` держит tenant-scoped DB context (`&Client`, `TenantId`).
//! Методы реализованы в `infrastructure::repos` (split impl pattern).
//! Handler создаёт repo один раз и вызывает методы без повторения client/tenant.

use bigdecimal::BigDecimal;
use kernel::types::TenantId;
use uuid::Uuid;

/// Строка баланса для query handler.
#[derive(Debug)]
pub struct BalanceRow {
    pub item_id: Uuid,
    pub sku: String,
    pub balance: BigDecimal,
}

/// Строка проекции товара из каталога.
#[derive(Debug)]
pub struct ProductProjectionRow {
    pub product_id: Uuid,
    pub name: String,
    pub category: String,
}

/// Tenant-scoped repository для складских агрегатов.
///
/// Создаётся в handler один раз, скрывает `client` и `tenant_id`:
/// ```ignore
/// let repo = InventoryRepo::new(db.client(), ctx.tenant_id);
/// let row = repo.find_by_sku(sku).await?;
/// ```
///
/// Методы реализованы в `infrastructure::repos` через clorinde.
pub struct InventoryRepo<'a> {
    pub(crate) client: &'a deadpool_postgres::Client,
    pub(crate) tenant_id: TenantId,
}

impl<'a> InventoryRepo<'a> {
    /// Создать tenant-scoped repo.
    #[must_use]
    pub fn new(client: &'a deadpool_postgres::Client, tenant_id: TenantId) -> Self {
        Self { client, tenant_id }
    }
}
