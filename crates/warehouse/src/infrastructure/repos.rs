//! `PgInventoryRepo` — SQL-доступ к складским данным через `clorinde_gen`.
//!
//! Ноль SQL в бизнес-коде — все запросы в `clorinde_gen::warehouse::*`.
//! Repo конвертирует между domain-типами (`BigDecimal`) и clorinde-типами (`String`).

use anyhow::Result;
use bigdecimal::BigDecimal;
use kernel::types::TenantId;
use std::str::FromStr;
use tokio_postgres::GenericClient;
use uuid::Uuid;

/// Строка баланса для query handler.
#[derive(Debug)]
pub struct BalanceRow {
    pub item_id: Uuid,
    pub sku: String,
    pub balance: BigDecimal,
}

/// Repository для складских агрегатов.
///
/// Делегирует SQL в `clorinde_gen::warehouse`, конвертирует типы.
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
        let row = clorinde_gen::warehouse::inventory::find_item_by_sku(
            client,
            *tenant_id.as_uuid(),
            sku,
        )
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
        clorinde_gen::warehouse::inventory::create_item(
            client,
            *tenant_id.as_uuid(),
            item_id,
            sku,
        )
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
        let qty_str = qty.to_string();
        let bal_str = balance_after.to_string();

        let params = clorinde_gen::warehouse::inventory::InsertMovementParams {
            tenant_id: *tenant_id.as_uuid(),
            id: movement_id,
            item_id,
            event_type,
            quantity: &qty_str,
            balance_after: &bal_str,
            doc_number,
            correlation_id,
            user_id,
        };
        clorinde_gen::warehouse::inventory::insert_movement(client, &params).await?;
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
        let bal_str = balance.to_string();

        let params = clorinde_gen::warehouse::balances::UpsertBalanceParams {
            tenant_id: *tenant_id.as_uuid(),
            item_id,
            sku,
            balance: &bal_str,
            last_movement_id: movement_id,
        };
        clorinde_gen::warehouse::balances::upsert_balance(client, &params).await?;
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
        let row = clorinde_gen::warehouse::balances::get_balance(
            client,
            *tenant_id.as_uuid(),
            sku,
        )
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
}
