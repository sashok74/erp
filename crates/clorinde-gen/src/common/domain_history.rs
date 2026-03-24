//! Типобезопасные запросы к `common.domain_history`.
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/common/domain_history.sql`.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Параметры для INSERT в `common.domain_history`.
pub struct InsertHistoryParams<'a> {
    pub tenant_id: Uuid,
    pub entity_type: &'a str,
    pub entity_id: Uuid,
    pub event_type: &'a str,
    pub old_state: Option<&'a serde_json::Value>,
    pub new_state: Option<&'a serde_json::Value>,
    pub correlation_id: Uuid,
    pub causation_id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// INSERT в `common.domain_history`, возвращает id.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn insert_domain_history(
    client: &impl tokio_postgres::GenericClient,
    params: &InsertHistoryParams<'_>,
) -> Result<i64, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO common.domain_history \
                (tenant_id, entity_type, entity_id, event_type, \
                 old_state, new_state, \
                 correlation_id, causation_id, user_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             RETURNING id",
            &[
                &params.tenant_id,
                &params.entity_type,
                &params.entity_id,
                &params.event_type,
                &params.old_state as &(dyn tokio_postgres::types::ToSql + Sync),
                &params.new_state as &(dyn tokio_postgres::types::ToSql + Sync),
                &params.correlation_id,
                &params.causation_id,
                &params.user_id,
                &params.created_at,
            ],
        )
        .await?;
    Ok(row.get(0))
}
