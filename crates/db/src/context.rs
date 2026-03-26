//! Convenience wrappers для доступа к БД в handler'ах.
//!
//! - `PgCommandContext` — для command handler'ов (внутри TX, через `PgUnitOfWork`)
//! - `ReadDbContext` — для query handler'ов (отдельное соединение + RLS)

use event_bus::EventEnvelope;
use kernel::entity::AggregateRoot;
use kernel::{AppError, DomainEvent, IntoInternal, RequestContext};
use runtime::ports::UnitOfWork;

use crate::pool::PgPool;
use crate::rls::set_tenant_context;
use crate::uow::PgUnitOfWork;

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

/// Контекст для query handler'ов — checkout + RLS в одну строку.
///
/// ```ignore
/// let db = ReadDbContext::acquire(&self.pool, ctx).await?;
/// let row = Repo::find(db.client(), ...).await?;
/// ```
pub struct ReadDbContext {
    client: deadpool_postgres::Object,
}

impl ReadDbContext {
    /// Взять соединение из pool'а и установить tenant context (RLS).
    ///
    /// # Errors
    ///
    /// `AppError::Internal` при ошибке checkout'а или `SET tenant_id`.
    pub async fn acquire(pool: &PgPool, ctx: &RequestContext) -> Result<Self, AppError> {
        let client = pool.get().await.internal("pool checkout")?;
        set_tenant_context(&**client, ctx.tenant_id)
            .await
            .internal("set tenant")?;
        Ok(Self { client })
    }

    /// `PostgreSQL`-клиент с установленным RLS-контекстом.
    pub fn client(&self) -> &deadpool_postgres::Object {
        &self.client
    }
}
