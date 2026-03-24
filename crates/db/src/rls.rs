//! RLS — Row-Level Security helpers.
//!
//! `SET LOCAL app.tenant_id` устанавливает tenant context внутри транзакции.
//! `PostgreSQL` RLS-политики используют `common.current_tenant_id()` для фильтрации.
//! `SET LOCAL` откатывается вместе с `ROLLBACK` — безопасно для connection pool.

use kernel::types::TenantId;

/// Установить tenant context на `PostgreSQL`-соединении (внутри TX).
///
/// Вызывать **после** `BEGIN`, **перед** любыми запросами к данным.
/// RLS-политики будут автоматически фильтровать строки по `tenant_id`.
///
/// # Errors
///
/// Ошибка если `SET LOCAL` не удался.
pub async fn set_tenant_context(
    client: &(impl tokio_postgres::GenericClient + Sync),
    tenant_id: TenantId,
) -> Result<(), anyhow::Error> {
    // SET LOCAL действует только до конца текущей транзакции.
    // Формат: SET LOCAL app.tenant_id = '<uuid>'
    // Используем format! — tenant_id валидный UUID, SQL injection невозможен.
    client
        .execute(
            &format!("SET LOCAL app.tenant_id = '{}'", tenant_id.as_uuid()),
            &[],
        )
        .await?;
    Ok(())
}
