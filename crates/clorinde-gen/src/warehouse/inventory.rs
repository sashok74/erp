//! Типобезопасные запросы к `warehouse.inventory_items` и `warehouse.stock_movements`.
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/warehouse/inventory.sql`.

use uuid::Uuid;

/// Строка из `find_item_by_sku` — id + balance как TEXT (NUMERIC → String).
#[derive(Debug, Clone)]
pub struct FindItemBySkuRow {
    pub id: Uuid,
    /// Balance как строка — caller конвертирует в `BigDecimal`.
    pub balance: String,
}

/// Параметры для INSERT в `warehouse.stock_movements`.
pub struct InsertMovementParams<'a> {
    pub tenant_id: Uuid,
    pub id: Uuid,
    pub item_id: Uuid,
    pub event_type: &'a str,
    pub quantity: &'a str,
    pub balance_after: &'a str,
    pub doc_number: &'a str,
    pub correlation_id: Uuid,
    pub user_id: Uuid,
}

/// Найти товар по SKU. Balance возвращается как TEXT.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn find_item_by_sku(
    client: &impl tokio_postgres::GenericClient,
    tenant_id: Uuid,
    sku: &str,
) -> Result<Option<FindItemBySkuRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT i.id, COALESCE(b.balance, 0)::TEXT AS balance \
             FROM warehouse.inventory_items i \
             LEFT JOIN warehouse.inventory_balances b \
               ON b.tenant_id = i.tenant_id AND b.item_id = i.id \
             WHERE i.tenant_id = $1 AND i.sku = $2",
            &[&tenant_id, &sku],
        )
        .await?;

    Ok(row.map(|r| FindItemBySkuRow {
        id: r.get(0),
        balance: r.get(1),
    }))
}

/// INSERT в `warehouse.inventory_items`.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn create_item(
    client: &impl tokio_postgres::GenericClient,
    tenant_id: Uuid,
    id: Uuid,
    sku: &str,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO warehouse.inventory_items (tenant_id, id, sku) \
             VALUES ($1, $2, $3)",
            &[&tenant_id, &id, &sku],
        )
        .await
}

/// INSERT в `warehouse.stock_movements`.
///
/// NUMERIC-поля передаются как TEXT, кастятся в SQL.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn insert_movement(
    client: &impl tokio_postgres::GenericClient,
    params: &InsertMovementParams<'_>,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO warehouse.stock_movements \
                (tenant_id, id, item_id, event_type, quantity, balance_after, \
                 doc_number, correlation_id, user_id) \
             VALUES ($1, $2, $3, $4, $5::TEXT::NUMERIC, $6::TEXT::NUMERIC, $7, $8, $9)",
            &[
                &params.tenant_id,
                &params.id,
                &params.item_id,
                &params.event_type,
                &params.quantity,
                &params.balance_after,
                &params.doc_number,
                &params.correlation_id,
                &params.user_id,
            ],
        )
        .await
}
