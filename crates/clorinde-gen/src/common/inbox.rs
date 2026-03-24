//! Типобезопасные запросы к `common.inbox` (event deduplication).
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/common/inbox.sql`.

use uuid::Uuid;

/// Попытка записать событие в inbox (idempotent).
///
/// Возвращает количество вставленных строк: 1 = новое, 0 = уже обработано.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn try_insert_inbox(
    client: &impl tokio_postgres::GenericClient,
    event_id: Uuid,
    event_type: &str,
    source: &str,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO common.inbox (event_id, event_type, source) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (event_id) DO NOTHING",
            &[&event_id, &event_type, &source],
        )
        .await
}

/// Проверить, было ли событие уже обработано.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn check_processed(
    client: &impl tokio_postgres::GenericClient,
    event_id: Uuid,
) -> Result<bool, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT 1 FROM common.inbox WHERE event_id = $1",
            &[&event_id],
        )
        .await?;
    Ok(row.is_some())
}
