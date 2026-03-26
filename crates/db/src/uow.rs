//! `PgUnitOfWork` — реализация `UnitOfWork` на `PostgreSQL`-транзакции.
//!
//! `BEGIN` → `SET LOCAL app.tenant_id` (RLS) → операции → INSERT outbox → `COMMIT`.
//!
//! # Lifetime-решение
//!
//! `UnitOfWork: Send + 'static` не совместим с `Transaction<'a>` (заимствует Client).
//! Используем owned `deadpool_postgres::Object` с ручным `BEGIN`/`COMMIT`/`ROLLBACK`.
//! Pipeline гарантирует вызов commit или rollback — утечки транзакций нет.

use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use event_bus::EventEnvelope;
use kernel::{AppError, RequestContext};
use runtime::ports::{UnitOfWork, UnitOfWorkFactory};
use tracing::debug;

use crate::pool::PgPool;
use crate::rls::set_tenant_context;

/// Фабрика `PgUnitOfWork`.
///
/// Хранит `Arc<PgPool>`, создаёт `PgUnitOfWork` для каждой команды.
pub struct PgUnitOfWorkFactory {
    pool: Arc<PgPool>,
}

impl PgUnitOfWorkFactory {
    /// Создать фабрику с указанным pool'ом.
    #[must_use]
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UnitOfWorkFactory for PgUnitOfWorkFactory {
    type UoW = PgUnitOfWork;

    async fn begin(&self, ctx: &RequestContext) -> Result<Self::UoW, AppError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| AppError::Internal(format!("pool checkout failed: {e}")))?;

        // Ручной BEGIN — потому что Transaction<'a> заимствует Client,
        // а UnitOfWork: Send + 'static требует owned данные.
        client
            .batch_execute("BEGIN")
            .await
            .map_err(|e| AppError::Internal(format!("BEGIN failed: {e}")))?;

        // RLS: устанавливаем tenant context внутри транзакции.
        // Deref Object → tokio_postgres::Client для GenericClient trait.
        set_tenant_context(&**client, ctx.tenant_id)
            .await
            .map_err(|e| AppError::Internal(format!("SET tenant_id failed: {e}")))?;

        debug!(
            tenant_id = %ctx.tenant_id.as_uuid(),
            "UoW started: BEGIN + SET tenant_id"
        );

        Ok(PgUnitOfWork {
            client,
            outbox_entries: Vec::new(),
        })
    }
}

/// Unit of Work на `PostgreSQL`-транзакции.
///
/// Owned `deadpool_postgres::Object` + ручной `BEGIN`/`COMMIT`/`ROLLBACK`.
/// Handler'ы получают доступ к client через `as_any_mut()` downcast.
pub struct PgUnitOfWork {
    /// Соединение из pool'а с активной транзакцией.
    client: deadpool_postgres::Object,
    /// Outbox-записи, накопленные handler'ом.
    outbox_entries: Vec<EventEnvelope>,
}

impl PgUnitOfWork {
    /// Доступ к `PostgreSQL`-клиенту для выполнения SQL внутри транзакции.
    ///
    /// Handler'ы используют для SQL-запросов:
    /// ```ignore
    /// let pg = uow.as_any_mut().downcast_mut::<PgUnitOfWork>().unwrap();
    /// let client = pg.client();
    /// client.query("SELECT ...", &[]).await?;
    /// ```
    pub fn client(&self) -> &deadpool_postgres::Object {
        &self.client
    }

    /// Мутабельный доступ к клиенту.
    pub fn client_mut(&mut self) -> &mut deadpool_postgres::Object {
        &mut self.client
    }

    /// Добавить outbox-запись напрямую (для `PgCommandContext`).
    pub(crate) fn push_outbox_entry(&mut self, envelope: EventEnvelope) {
        self.outbox_entries.push(envelope);
    }

    /// INSERT всех outbox entries в `common.outbox` (внутри текущей TX).
    async fn flush_outbox(&self) -> Result<(), AppError> {
        for entry in &self.outbox_entries {
            let tenant_id = *entry.tenant_id.as_uuid();
            let user_id = *entry.user_id.as_uuid();
            let created_at = entry.timestamp.fixed_offset();
            clorinde_gen::queries::common::outbox::insert_outbox_entry()
                .bind(
                    &self.client,
                    &tenant_id,
                    &entry.event_id,
                    &entry.event_type,
                    &entry.source,
                    &entry.payload,
                    &entry.correlation_id,
                    &entry.causation_id,
                    &user_id,
                    &created_at,
                )
                .one()
                .await
                .map_err(|e| AppError::Internal(format!("outbox INSERT failed: {e}")))?;
        }
        Ok(())
    }
}

#[async_trait]
impl UnitOfWork for PgUnitOfWork {
    fn add_outbox_entry(&mut self, envelope: EventEnvelope) {
        self.outbox_entries.push(envelope);
    }

    async fn commit(self: Box<Self>) -> Result<(), AppError> {
        // Сначала записываем outbox — в той же транзакции.
        self.flush_outbox().await?;

        self.client
            .batch_execute("COMMIT")
            .await
            .map_err(|e| AppError::Internal(format!("COMMIT failed: {e}")))?;

        debug!(
            outbox_count = self.outbox_entries.len(),
            "UoW committed with outbox entries"
        );
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> Result<(), AppError> {
        self.client
            .batch_execute("ROLLBACK")
            .await
            .map_err(|e| AppError::Internal(format!("ROLLBACK failed: {e}")))?;

        debug!("UoW rolled back");
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
