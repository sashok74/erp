//! `DomainHistoryWriter` — запись old/new state при каждом изменении сущности.
//!
//! В отличие от audit log (кто/когда/какая команда), domain history фиксирует
//! **что именно изменилось** в данных.
//!
//! Вызывается **внутри `UoW` TX** — атомарно с domain data.
//! Handler получает writer и вызывает `record()` через то же соединение.

use kernel::RequestContext;
use uuid::Uuid;

/// Writer для domain history — снимки old/new state.
///
/// Handler вызывает `record()` через `tokio_postgres::GenericClient`
/// из `PgUnitOfWork`, обеспечивая атомарность с domain data.
pub struct DomainHistoryWriter;

impl DomainHistoryWriter {
    /// Записать изменение сущности в `common.domain_history`.
    ///
    /// Вызывается внутри `UoW` TX — клиент получен через downcast.
    ///
    /// # Errors
    ///
    /// `anyhow::Error` при ошибке SQL.
    pub async fn record(
        client: &impl tokio_postgres::GenericClient,
        ctx: &RequestContext,
        entity_type: &str,
        entity_id: Uuid,
        event_type: &str,
        old_state: Option<&serde_json::Value>,
        new_state: Option<&serde_json::Value>,
    ) -> Result<i64, anyhow::Error> {
        let now = chrono::Utc::now();
        let row = client
            .query_one(
                "INSERT INTO common.domain_history \
                    (tenant_id, entity_type, entity_id, event_type, \
                     old_state, new_state, \
                     correlation_id, causation_id, user_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
                 RETURNING id",
                &[
                    ctx.tenant_id.as_uuid(),
                    &entity_type,
                    &entity_id,
                    &event_type,
                    &old_state as &(dyn tokio_postgres::types::ToSql + Sync),
                    &new_state as &(dyn tokio_postgres::types::ToSql + Sync),
                    &ctx.correlation_id,
                    &ctx.causation_id,
                    ctx.user_id.as_uuid(),
                    &now,
                ],
            )
            .await?;
        Ok(row.get(0))
    }
}
