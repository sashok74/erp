//! `PgInventoryRepo` — SQL-доступ к складским данным через `clorinde_gen`.
//!
//! Ноль SQL в бизнес-коде — все запросы в `clorinde_gen::queries::warehouse::*`.
//! Конверсии типов — через `db::conversions` helpers.

use anyhow::Result;
use bigdecimal::BigDecimal;
use clorinde_gen::client::GenericClient;
use db::conversions::{parse_dec, tid};
use db::transport::DecStr;
use db::{repo_exec, repo_opt};
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
        let row = clorinde_gen::queries::warehouse::inventory::find_item_by_sku()
            .bind(client, &tid(tenant_id), &sku)
            .opt()
            .await?;

        match row {
            Some(r) => Ok(Some((r.id, parse_dec(&r.balance)?))),
            None => Ok(None),
        }
    }

    repo_exec! {
        /// Создать запись `inventory_item`.
        pub async fn create_item(
            client: &impl GenericClient,
            tenant_id: TenantId,
            item_id: Uuid,
            sku: &str,
        ) via clorinde_gen::queries::warehouse::inventory::create_item;
        bind = [&tid(tenant_id), &item_id, &sku];
    }

    repo_exec! {
        /// Записать движение товара (append-only).
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
        ) via clorinde_gen::queries::warehouse::inventory::insert_movement;
        bind = [
            &tid(tenant_id), &movement_id, &item_id, &event_type,
            &DecStr(qty), &DecStr(balance_after), &doc_number,
            &correlation_id, &user_id
        ];
    }

    repo_exec! {
        /// Upsert текущего баланса (INSERT ON CONFLICT UPDATE).
        pub async fn upsert_balance(
            client: &impl GenericClient,
            tenant_id: TenantId,
            item_id: Uuid,
            sku: &str,
            balance: &BigDecimal,
            movement_id: Uuid,
        ) via clorinde_gen::queries::warehouse::balances::upsert_balance;
        bind = [&tid(tenant_id), &item_id, &sku, &DecStr(balance), &movement_id];
    }

    repo_opt! {
        /// Получить баланс по SKU (для query handler).
        pub async fn get_balance(
            client: &impl GenericClient,
            tenant_id: TenantId,
            sku: &str,
        ) -> Option<BalanceRow>
        via clorinde_gen::queries::warehouse::balances::get_balance;
        bind = [&tid(tenant_id), &sku];
        map = |r| {
            BalanceRow {
                item_id: r.item_id,
                sku: r.sku,
                balance: parse_dec(&r.balance)?,
            }
        };
    }

    repo_opt! {
        /// Получить проекцию товара по SKU (из `warehouse.product_projections`).
        pub async fn get_product_projection(
            client: &impl GenericClient,
            tenant_id: TenantId,
            sku: &str,
        ) -> Option<ProductProjectionRow>
        via clorinde_gen::queries::warehouse::projections::get_projection_by_sku;
        bind = [&tid(tenant_id), &sku];
        map = |r| {
            ProductProjectionRow {
                product_id: r.product_id,
                name: r.name,
                category: r.category,
            }
        };
    }
}
