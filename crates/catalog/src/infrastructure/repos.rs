//! `PgProductRepo` — SQL-доступ к каталогу товаров через `clorinde_gen`.

use anyhow::Result;
use clorinde_gen::client::GenericClient;
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
    /// Найти товар по SKU.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса.
    pub async fn find_by_sku(
        client: &impl GenericClient,
        tenant_id: TenantId,
        sku: &str,
    ) -> Result<Option<ProductRow>> {
        let tid = *tenant_id.as_uuid();
        let row = clorinde_gen::queries::catalog::products::find_by_sku()
            .bind(client, &tid, &sku)
            .opt()
            .await?;

        Ok(row.map(|r| ProductRow {
            id: r.id,
            sku: r.sku,
            name: r.name,
            category: r.category,
            unit: r.unit,
        }))
    }

    /// Создать товар в каталоге.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса.
    pub async fn create_product(
        client: &impl GenericClient,
        tenant_id: TenantId,
        id: Uuid,
        sku: &str,
        name: &str,
        category: &str,
        unit: &str,
    ) -> Result<()> {
        let tid = *tenant_id.as_uuid();
        clorinde_gen::queries::catalog::products::create_product()
            .bind(client, &tid, &id, &sku, &name, &category, &unit)
            .await?;
        Ok(())
    }
}
