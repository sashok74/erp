//! `PgProductRepo` — SQL-доступ к каталогу товаров через `clorinde_gen`.

use db::conversions::tid;
use db::{repo_exec, repo_opt};
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

/// Repository для товаров каталога.
pub struct PgProductRepo;

impl PgProductRepo {
    repo_opt! {
        /// Найти товар по SKU.
        pub async fn find_by_sku(
            client: &impl GenericClient,
            tenant_id: TenantId,
            sku: &str,
        ) -> Option<ProductRow>
        via clorinde_gen::queries::catalog::products::find_by_sku;
        bind = [&tid(tenant_id), &sku];
        map = |r| {
            ProductRow {
                id: r.id,
                sku: r.sku,
                name: r.name,
                category: r.category,
                unit: r.unit,
            }
        };
    }

    repo_exec! {
        /// Создать товар в каталоге.
        pub async fn create_product(
            client: &impl GenericClient,
            tenant_id: TenantId,
            id: Uuid,
            sku: &str,
            name: &str,
            category: &str,
            unit: &str,
        ) via clorinde_gen::queries::catalog::products::create_product;
        bind = [&tid(tenant_id), &id, &sku, &name, &category, &unit];
    }
}
