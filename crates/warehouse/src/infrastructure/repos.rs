//! Реализации persistence для `InventoryRepo` через `clorinde_gen`.
//!
//! Split impl: struct определён в `application::ports`,
//! методы реализованы здесь через clorinde SQL.
//! Все методы — `&self`, `client` и `tenant_id` берутся из repo.

use bigdecimal::BigDecimal;
use db::conversions::{parse_dec, tid};
use db::transport::DecStr;
use db::{repo_exec, repo_opt};
use kernel::{AppError, IntoInternal};
use uuid::Uuid;

use crate::application::ports::{BalanceRow, InventoryRepo, ProductProjectionRow};

impl InventoryRepo<'_> {
    /// Найти товар по SKU. Возвращает (`item_id`, balance).
    ///
    /// # Errors
    ///
    /// `AppError::Internal` при сбое SQL-запроса или парсинге баланса.
    pub async fn find_by_sku(&self, sku: &str) -> Result<Option<(Uuid, BigDecimal)>, AppError> {
        let row = clorinde_gen::queries::warehouse::inventory::find_item_by_sku()
            .bind(self.client, &tid(self.tenant_id), &sku)
            .opt()
            .await
            .internal("find_by_sku")?;

        match row {
            Some(r) => Ok(Some((r.id, parse_dec(&r.balance).internal("find_by_sku")?))),
            None => Ok(None),
        }
    }

    repo_exec! {
        /// Создать запись `inventory_item`.
        pub async fn create_item(
            item_id: Uuid,
            sku: &str,
        ) via clorinde_gen::queries::warehouse::inventory::create_item;
        bind = [&item_id, &sku];
    }

    repo_exec! {
        /// Записать движение товара (append-only).
        pub async fn save_movement(
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
            &movement_id, &item_id, &event_type,
            &DecStr(qty), &DecStr(balance_after), &doc_number,
            &correlation_id, &user_id
        ];
    }

    repo_exec! {
        /// Upsert текущего баланса (INSERT ON CONFLICT UPDATE).
        pub async fn upsert_balance(
            item_id: Uuid,
            sku: &str,
            balance: &BigDecimal,
            movement_id: Uuid,
        ) via clorinde_gen::queries::warehouse::balances::upsert_balance;
        bind = [&item_id, &sku, &DecStr(balance), &movement_id];
    }

    repo_opt! {
        /// Получить баланс по SKU (для query handler).
        pub async fn get_balance(
            sku: &str,
        ) -> Option<BalanceRow>
        via clorinde_gen::queries::warehouse::balances::get_balance;
        bind = [&sku];
        map = |r| {
            BalanceRow {
                item_id: r.item_id,
                sku: r.sku,
                balance: parse_dec(&r.balance).internal("get_balance")?,
            }
        };
    }

    repo_opt! {
        /// Получить проекцию товара по SKU (из `warehouse.product_projections`).
        pub async fn get_product_projection(
            sku: &str,
        ) -> Option<ProductProjectionRow>
        via clorinde_gen::queries::warehouse::projections::get_projection_by_sku;
        bind = [&sku];
        map = |r| {
            ProductProjectionRow {
                product_id: r.product_id,
                name: r.name,
                category: r.category,
            }
        };
    }

    repo_exec! {
        /// Upsert проекции товара из каталога (event handler).
        pub async fn upsert_product_projection(
            product_id: Uuid,
            sku: &str,
            name: &str,
            category: &str,
        ) via clorinde_gen::queries::warehouse::projections::upsert_product_projection;
        bind = [&product_id, &sku, &name, &category];
    }
}
