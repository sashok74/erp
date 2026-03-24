//! Типобезопасные запросы к `common.outbox`.
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/common/outbox.sql`.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Строка из `common.outbox` — неопубликованное событие.
#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: i64,
    pub tenant_id: Uuid,
    pub event_id: Uuid,
    pub event_type: String,
    pub source: String,
    pub payload: serde_json::Value,
    pub correlation_id: Uuid,
    pub causation_id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub retry_count: i32,
}

/// Параметры для INSERT в `common.outbox`.
pub struct InsertOutboxParams<'a> {
    pub tenant_id: Uuid,
    pub event_id: Uuid,
    pub event_type: &'a str,
    pub source: &'a str,
    pub payload: &'a serde_json::Value,
    pub correlation_id: Uuid,
    pub causation_id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// INSERT в `common.outbox`, возвращает id.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn insert_outbox_entry(
    client: &impl tokio_postgres::GenericClient,
    params: &InsertOutboxParams<'_>,
) -> Result<i64, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO common.outbox \
                (tenant_id, event_id, event_type, source, payload, \
                 correlation_id, causation_id, user_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             RETURNING id",
            &[
                &params.tenant_id,
                &params.event_id,
                &params.event_type,
                &params.source,
                &params.payload,
                &params.correlation_id,
                &params.causation_id,
                &params.user_id,
                &params.created_at,
            ],
        )
        .await?;
    Ok(row.get(0))
}

/// Получить batch неопубликованных событий с блокировкой (FOR UPDATE SKIP LOCKED).
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn get_unpublished_events(
    client: &impl tokio_postgres::GenericClient,
    batch_size: i64,
) -> Result<Vec<OutboxRow>, tokio_postgres::Error> {
    let rows = client
        .query(
            "SELECT id, tenant_id, event_id, event_type, source, payload, \
                    correlation_id, causation_id, user_id, created_at, retry_count \
             FROM common.outbox \
             WHERE published = false \
             ORDER BY id \
             LIMIT $1 \
             FOR UPDATE SKIP LOCKED",
            &[&batch_size],
        )
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| OutboxRow {
            id: r.get(0),
            tenant_id: r.get(1),
            event_id: r.get(2),
            event_type: r.get(3),
            source: r.get(4),
            payload: r.get(5),
            correlation_id: r.get(6),
            causation_id: r.get(7),
            user_id: r.get(8),
            created_at: r.get(9),
            retry_count: r.get(10),
        })
        .collect())
}

/// Пометить событие как опубликованное.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn mark_published(
    client: &impl tokio_postgres::GenericClient,
    id: i64,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "UPDATE common.outbox SET published = true, published_at = NOW() WHERE id = $1",
            &[&id],
        )
        .await
}

/// Инкрементировать счётчик повторных попыток.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn increment_retry(
    client: &impl tokio_postgres::GenericClient,
    id: i64,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "UPDATE common.outbox SET retry_count = retry_count + 1 WHERE id = $1",
            &[&id],
        )
        .await
}
