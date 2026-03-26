//! `DomainHistoryWriter` — запись old/new state при каждом изменении сущности.
//!
//! В отличие от audit log (кто/когда/какая команда), domain history фиксирует
//! **что именно изменилось** в данных.
//!
//! Вызывается **внутри `UoW` TX** — атомарно с domain data.
//! Handler получает writer и вызывает `record()` через то же соединение.

use clorinde_gen::client::GenericClient;
use kernel::{AppError, IntoInternal, RequestContext};
use serde::Serialize;
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

    /// Записать изменение с автоматической сериализацией old/new state.
    ///
    /// Обёртка над `record()`: принимает `&impl Serialize` вместо `&serde_json::Value`,
    /// маппит ошибки в `AppError::Internal`.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` при ошибке сериализации или SQL.
    pub async fn record_change<O: Serialize, N: Serialize>(
        client: &impl GenericClient,
        ctx: &RequestContext,
        entity_type: &str,
        entity_id: Uuid,
        event_type: &str,
        old: Option<&O>,
        new: Option<&N>,
    ) -> Result<i64, AppError> {
        let old_val = old
            .map(serde_json::to_value)
            .transpose()
            .internal("serialize old_state")?;
        let new_val = new
            .map(serde_json::to_value)
            .transpose()
            .internal("serialize new_state")?;
        Self::record(
            client,
            ctx,
            entity_type,
            entity_id,
            event_type,
            old_val.as_ref(),
            new_val.as_ref(),
        )
        .await
        .internal("domain_history")
    }
}
