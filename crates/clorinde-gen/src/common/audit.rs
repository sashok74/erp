//! Типобезопасные запросы к `common.audit_log`.
//!
//! TODO: заменить на автогенерацию Clorinde CLI из `queries/common/audit.sql`.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Параметры для INSERT в `common.audit_log`.
pub struct InsertAuditParams<'a> {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub command_name: &'a str,
    pub result: &'a serde_json::Value,
    pub correlation_id: Uuid,
    pub causation_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// INSERT в `common.audit_log`, возвращает id.
///
/// # Errors
///
/// `tokio_postgres::Error` при ошибке SQL.
pub async fn insert_audit_log(
    client: &impl tokio_postgres::GenericClient,
    params: &InsertAuditParams<'_>,
) -> Result<i64, tokio_postgres::Error> {
    let row = client
        .query_one(
            "INSERT INTO common.audit_log \
                (tenant_id, user_id, command_name, result, \
                 correlation_id, causation_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING id",
            &[
                &params.tenant_id,
                &params.user_id,
                &params.command_name,
                &params.result,
                &params.correlation_id,
                &params.causation_id,
                &params.created_at,
            ],
        )
        .await?;
    Ok(row.get(0))
}
