//! `DomainHistoryWriter` — запись old/new state при каждом изменении сущности.
//!
//! В отличие от audit log (кто/когда/какая команда), domain history фиксирует
//! **что именно изменилось** в данных.
//!
//! Вызывается **внутри `UoW` TX** — атомарно с domain data.
//! Handler получает writer и вызывает `record()` через то же соединение.

use clorinde_gen::client::GenericClient;
use kernel::RequestContext;
use uuid::Uuid;

/// Writer для domain history — снимки old/new state.
///
/// Handler вызывает `record()` через `clorinde_gen::client::async_::GenericClient`
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
        client: &impl GenericClient,
        ctx: &RequestContext,
        entity_type: &str,
        entity_id: Uuid,
        event_type: &str,
        old_state: Option<&serde_json::Value>,
        new_state: Option<&serde_json::Value>,
    ) -> Result<i64, anyhow::Error> {
        let now = chrono::Utc::now().fixed_offset();
        let tenant_id = *ctx.tenant_id.as_uuid();
        let user_id = *ctx.user_id.as_uuid();
        // Convert Option<&Value> to serde_json::Value for JsonSql trait compatibility.
        let old_state_val = old_state
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let new_state_val = new_state
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let id = clorinde_gen::queries::common::domain_history::insert_domain_history()
            .bind(
                client,
                &tenant_id,
                &entity_type,
                &entity_id,
                &event_type,
                &old_state_val,
                &new_state_val,
                &ctx.correlation_id,
                &ctx.causation_id,
                &user_id,
                &now,
            )
            .one()
            .await?;
        Ok(id)
    }
}
