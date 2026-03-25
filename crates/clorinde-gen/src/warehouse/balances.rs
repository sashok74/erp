//! Типобезопасные запросы к `warehouse.inventory_balances`.
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/warehouse/balances.sql`.

use uuid::Uuid;

/// Строка из `get_balance` — balance как TEXT.
#[derive(Debug, Clone)]
pub struct BalanceRow {
    pub item_id: Uuid,
    pub sku: String,
    /// Balance как строка — caller конвертирует в `BigDecimal`.
    pub balance: String,
}

/// Параметры для UPSERT в `warehouse.inventory_balances`.
pub struct UpsertBalanceParams<'a> {
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub sku: &'a str,
    /// Balance как строка (TEXT → NUMERIC cast в SQL).
    pub balance: &'a str,
    pub last_movement_id: Uuid,
}

/// UPSERT текущего баланса (INSERT ON CONFLICT UPDATE).
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn upsert_balance(
    client: &impl tokio_postgres::GenericClient,
    params: &UpsertBalanceParams<'_>,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO warehouse.inventory_balances \
                (tenant_id, item_id, sku, balance, last_movement_id, updated_at) \
             VALUES ($1, $2, $3, $4::TEXT::NUMERIC, $5, now()) \
             ON CONFLICT (tenant_id, item_id) DO UPDATE SET \
                balance = $4::TEXT::NUMERIC, \
                last_movement_id = $5, \
                updated_at = now()",
            &[
                &params.tenant_id,
                &params.item_id,
                &params.sku,
                &params.balance,
                &params.last_movement_id,
            ],
        )
        .await
}

/// Получить баланс по SKU. Balance возвращается как TEXT.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn get_balance(
    client: &impl tokio_postgres::GenericClient,
    tenant_id: Uuid,
    sku: &str,
) -> Result<Option<BalanceRow>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT item_id, sku, balance::TEXT \
             FROM warehouse.inventory_balances \
             WHERE tenant_id = $1 AND sku = $2",
            &[&tenant_id, &sku],
        )
        .await?;

    Ok(row.map(|r| BalanceRow {
        item_id: r.get(0),
        sku: r.get(1),
        balance: r.get(2),
    }))
}
