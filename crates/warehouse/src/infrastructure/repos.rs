//! `PgInventoryRepo` — SQL-доступ к складским данным через `clorinde_gen`.
//!
//! Ноль SQL в бизнес-коде — все запросы в `clorinde_gen::queries::warehouse::*`.
//! Repo конвертирует между domain-типами (`BigDecimal`) и clorinde-типами (`String`).

use anyhow::Result;
use bigdecimal::BigDecimal;
use clorinde_gen::client::GenericClient;
use kernel::types::TenantId;
use std::str::FromStr;
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

/// Repository для складских агрегатов.
///
/// Делегирует SQL в `clorinde_gen::queries::warehouse`, конвертирует типы.
pub struct PgInventoryRepo;

impl PgInventoryRepo {
    /// Найти товар по SKU. Возвращает (`item_id`, balance).
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса или парсинге баланса.
    pub async fn find_by_sku(
        client: &impl GenericClient,
        tenant_id: TenantId,
        sku: &str,
    ) -> Result<Option<(Uuid, BigDecimal)>> {
        let tid = *tenant_id.as_uuid();
        let row = clorinde_gen::queries::warehouse::inventory::find_item_by_sku()
            .bind(client, &tid, &sku)
            .opt()
            .await?;

        match row {
            Some(r) => {
                let balance = BigDecimal::from_str(&r.balance)
                    .map_err(|e| anyhow::anyhow!("parse balance: {e}"))?;
                Ok(Some((r.id, balance)))
            }
            None => Ok(None),
        }
    }

    /// Создать запись `inventory_item`.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса.
    pub async fn create_item(
        client: &impl GenericClient,
        tenant_id: TenantId,
        item_id: Uuid,
        sku: &str,
    ) -> Result<()> {
        let tid = *tenant_id.as_uuid();
        clorinde_gen::queries::warehouse::inventory::create_item()
            .bind(client, &tid, &item_id, &sku)
            .await?;
        Ok(())
    }

    /// Записать движение товара (append-only).
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_movement(
        client: &impl GenericClient,
        tenant_id: TenantId,
        movement_id: Uuid,
        item_id: Uuid,
        event_type: &str,
        qty: &BigDecimal,
        balance_after: &BigDecimal,
        doc_number: &str,
        correlation_id: Uuid,
        user_id: Uuid,
    ) -> Result<()> {
        let tid = *tenant_id.as_uuid();
        let qty_str = qty.to_string();
        let bal_str = balance_after.to_string();

        clorinde_gen::queries::warehouse::inventory::insert_movement()
            .bind(
                client,
                &tid,
                &movement_id,
                &item_id,
                &event_type,
                &qty_str,
                &bal_str,
                &doc_number,
                &correlation_id,
                &user_id,
            )
            .await?;
        Ok(())
    }

    /// Upsert текущего баланса (INSERT ON CONFLICT UPDATE).
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса.
    pub async fn upsert_balance(
        client: &impl GenericClient,
        tenant_id: TenantId,
        item_id: Uuid,
        sku: &str,
        balance: &BigDecimal,
        movement_id: Uuid,
    ) -> Result<()> {
        let tid = *tenant_id.as_uuid();
        let bal_str = balance.to_string();

        clorinde_gen::queries::warehouse::balances::upsert_balance()
            .bind(client, &tid, &item_id, &sku, &bal_str, &movement_id)
            .await?;
        Ok(())
    }

    /// Получить баланс по SKU (для query handler).
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса или парсинге баланса.
    pub async fn get_balance(
        client: &impl GenericClient,
        tenant_id: TenantId,
        sku: &str,
    ) -> Result<Option<BalanceRow>> {
        let tid = *tenant_id.as_uuid();
        let row = clorinde_gen::queries::warehouse::balances::get_balance()
            .bind(client, &tid, &sku)
            .opt()
            .await?;

        match row {
            Some(r) => {
                let balance = BigDecimal::from_str(&r.balance)
                    .map_err(|e| anyhow::anyhow!("parse balance: {e}"))?;
                Ok(Some(BalanceRow {
                    item_id: r.item_id,
                    sku: r.sku,
                    balance,
                }))
            }
            None => Ok(None),
        }
    }

    /// Получить проекцию товара по SKU (из `warehouse.product_projections`).
    ///
    /// # Errors
    ///
    /// Возвращает ошибку при сбое SQL-запроса.
    pub async fn get_product_projection(
        client: &impl GenericClient,
        tenant_id: TenantId,
        sku: &str,
    ) -> Result<Option<ProductProjectionRow>> {
        let tid = *tenant_id.as_uuid();
        let row = clorinde_gen::queries::warehouse::projections::get_projection_by_sku()
            .bind(client, &tid, &sku)
            .opt()
            .await?;

        Ok(row.map(|r| ProductProjectionRow {
            product_id: r.product_id,
            name: r.name,
            category: r.category,
        }))
    }
}
