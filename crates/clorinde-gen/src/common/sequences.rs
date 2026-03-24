//! Типобезопасные запросы к `common.sequences`.
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/common/sequences.sql`.

use uuid::Uuid;

/// Результат `next_value` — текущее значение последовательности.
#[derive(Debug, Clone)]
pub struct SequenceValue {
    pub prefix: String,
    pub next_value: i64,
}

/// Получить текущее значение последовательности (FOR UPDATE — блокировка строки).
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn next_value(
    client: &impl tokio_postgres::GenericClient,
    tenant_id: Uuid,
    seq_name: &str,
) -> Result<Option<SequenceValue>, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT prefix, next_value \
             FROM common.sequences \
             WHERE tenant_id = $1 AND seq_name = $2 \
             FOR UPDATE",
            &[&tenant_id, &seq_name],
        )
        .await?;

    Ok(row.map(|r| SequenceValue {
        prefix: r.get(0),
        next_value: r.get(1),
    }))
}

/// Инкрементировать значение последовательности.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn increment_sequence(
    client: &impl tokio_postgres::GenericClient,
    tenant_id: Uuid,
    seq_name: &str,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "UPDATE common.sequences SET next_value = next_value + 1 \
             WHERE tenant_id = $1 AND seq_name = $2",
            &[&tenant_id, &seq_name],
        )
        .await
}

/// Создать последовательность, если не существует (INSERT ON CONFLICT DO NOTHING).
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn ensure_sequence(
    client: &impl tokio_postgres::GenericClient,
    tenant_id: Uuid,
    seq_name: &str,
    prefix: &str,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO common.sequences (tenant_id, seq_name, prefix, next_value) \
             VALUES ($1, $2, $3, 1) \
             ON CONFLICT (tenant_id, seq_name) DO NOTHING",
            &[&tenant_id, &seq_name, &prefix],
        )
        .await
}
