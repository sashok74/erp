//! Convenience wrappers для доступа к БД в command handler'ах.
//!
//! `PgCommandContext` — доступ к `PgUnitOfWork` client + helpers
//! (`record_change`, `emit_events`). Для read path используйте [`ReadScope`].

use event_bus::EventEnvelope;
use kernel::entity::AggregateRoot;
use kernel::{AppError, DomainEvent, IntoInternal, RequestContext};
use runtime::ports::UnitOfWork;
use serde::Serialize;
use uuid::Uuid;

use crate::uow::{PendingHistoryEntry, PgUnitOfWork};

/// Контекст для command handler'ов — доступ к `PgUnitOfWork` client + helpers.
///
/// Заменяет ручной downcast + split-borrow dance:
///
/// ```ignore
/// let mut db = PgCommandContext::from_uow(uow)?;
/// let client = db.client();
/// // ... SQL через client ...
/// db.emit_events(&mut aggregate, ctx, "warehouse")?;
/// ```
pub struct PgCommandContext<'a> {
    inner: &'a mut PgUnitOfWork,
}

impl<'a> PgCommandContext<'a> {
    /// Downcast `dyn UnitOfWork` → `PgUnitOfWork`.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` если `uow` не является `PgUnitOfWork`.
    #[must_use = "PgCommandContext provides access to the DB client"]
    pub fn from_uow(uow: &'a mut dyn UnitOfWork) -> Result<Self, AppError> {
        let inner = uow
            .as_any_mut()
            .downcast_mut::<PgUnitOfWork>()
            .ok_or_else(|| AppError::Internal("expected PgUnitOfWork".into()))?;
        Ok(Self { inner })
    }

    /// `PostgreSQL`-клиент внутри активной транзакции.
    pub fn client(&self) -> &deadpool_postgres::Object {
        self.inner.client()
    }

    /// Зарегистрировать изменение сущности для domain history (deferred).
    ///
    /// Не делает I/O — только сериализует и добавляет в pending list.
    /// Фактический INSERT произойдёт в `UoW::commit()`.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` при ошибке сериализации old/new state.
    pub fn record_change<O: Serialize, N: Serialize>(
        &mut self,
        ctx: &RequestContext,
        entity_type: &str,
        entity_id: Uuid,
        event_type: &str,
        old: Option<&O>,
        new: Option<&N>,
    ) -> Result<(), AppError> {
        let old_state = old
            .map(serde_json::to_value)
            .transpose()
            .internal("serialize old_state")?
            .unwrap_or(serde_json::Value::Null);
        let new_state = new
            .map(serde_json::to_value)
            .transpose()
            .internal("serialize new_state")?
            .unwrap_or(serde_json::Value::Null);

        let entry = PendingHistoryEntry {
            tenant_id: *ctx.tenant_id.as_uuid(),
            entity_type: entity_type.to_string(),
            entity_id,
            event_type: event_type.to_string(),
            old_state,
            new_state,
            correlation_id: ctx.correlation_id,
            causation_id: ctx.causation_id,
            user_id: *ctx.user_id.as_uuid(),
            created_at: chrono::Utc::now().fixed_offset(),
        };
        self.inner.push_history_entry(entry);
        Ok(())
    }

    /// Забрать events из агрегата → `EventEnvelope` → outbox.
    ///
    /// Заменяет: `take_events() → for → from_domain_event → push_outbox_entry`.
    ///
    /// # Errors
    ///
    /// `AppError::Internal` при ошибке сериализации события.
    pub fn emit_events<A>(
        &mut self,
        aggregate: &mut A,
        ctx: &RequestContext,
        source: &str,
    ) -> Result<(), AppError>
    where
        A: AggregateRoot,
        A::Event: DomainEvent,
    {
        let events = aggregate.take_events();
        for evt in &events {
            let envelope =
                EventEnvelope::from_domain_event(evt, ctx, source).internal("serialize event")?;
            self.inner.push_outbox_entry(envelope);
        }
        Ok(())
    }
}
