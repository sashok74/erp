//! Inbox dedup — exactly-once event processing через `common.inbox`.
//!
//! `InboxGuard` — low-level API для проверки/записи inbox.
//! `InboxAwareHandler` — decorator, оборачивающий handler в check → handle → mark.

use std::sync::Arc;

use async_trait::async_trait;
use event_bus::EventEnvelope;
use event_bus::registry::ErasedEventHandler;
use uuid::Uuid;

use crate::pool::PgPool;

/// Guard для exactly-once обработки событий.
///
/// Low-level API. В production-коде используется `InboxAwareHandler`,
/// который вызывает `is_processed` и `mark_processed` автоматически.
pub struct InboxGuard {
    pool: Arc<PgPool>,
}

impl InboxGuard {
    /// Создать guard с указанным pool.
    #[must_use]
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Зарегистрировать успешную обработку события конкретным handler'ом.
    ///
    /// Возвращает `true` если запись создана (первая обработка).
    /// Возвращает `false` если уже обработано (duplicate).
    ///
    /// # Errors
    ///
    /// `anyhow::Error` при ошибке SQL.
    pub async fn mark_processed(
        &self,
        event_id: Uuid,
        event_type: &str,
        source: &str,
        handler_name: &str,
    ) -> Result<bool, anyhow::Error> {
        let client = self.pool.get().await?;
        let rows_affected = clorinde_gen::queries::common::inbox::try_insert_inbox()
            .bind(&client, &event_id, &event_type, &source, &handler_name)
            .await?;
        Ok(rows_affected == 1)
    }

    /// Проверить, обработано ли событие конкретным handler'ом.
    ///
    /// # Errors
    ///
    /// `anyhow::Error` при ошибке SQL.
    pub async fn is_processed(
        &self,
        event_id: Uuid,
        handler_name: &str,
    ) -> Result<bool, anyhow::Error> {
        let client = self.pool.get().await?;
        let row = clorinde_gen::queries::common::inbox::check_processed()
            .bind(&client, &event_id, &handler_name)
            .opt()
            .await?;
        Ok(row.is_some())
    }
}

/// Decorator: inbox dedup → inner handler.
///
/// Lifecycle: check → handle → mark (НЕ claim-before-handle).
///
/// 1. SELECT — уже обработано этим handler'ом? → skip
/// 2. `inner.handle_envelope()` — если Err, return Err (inbox не записан → retry)
/// 3. INSERT inbox — фиксируем успешную обработку
///
/// ## Concurrency scope
///
/// Dedup guarantee scoped to relay-delivered events.
/// `OutboxRelay` — единственный publisher, sequential `publish_and_wait`.
/// Concurrent delivery одного `(event_id, handler_name)` невозможна
/// в рамках этого architectural invariant.
pub struct InboxAwareHandler {
    inner: Arc<dyn ErasedEventHandler>,
    inbox: Arc<InboxGuard>,
}

impl InboxAwareHandler {
    pub fn new(inner: Arc<dyn ErasedEventHandler>, inbox: Arc<InboxGuard>) -> Self {
        Self { inner, inbox }
    }
}

#[async_trait]
impl ErasedEventHandler for InboxAwareHandler {
    async fn handle_envelope(&self, envelope: &EventEnvelope) -> Result<(), anyhow::Error> {
        let handler_name = self.inner.handler_name();

        // 1. Check: уже обработано этим handler'ом?
        if self.inbox.is_processed(envelope.event_id, handler_name).await? {
            tracing::debug!(
                event_id = %envelope.event_id,
                event_type = %envelope.event_type,
                handler = handler_name,
                "inbox: duplicate event, skipping"
            );
            return Ok(());
        }

        // 2. Handle: вызываем inner handler.
        //    Если handler упал — inbox НЕ записан → relay retry пройдёт.
        self.inner.handle_envelope(envelope).await?;

        // 3. Mark: фиксируем успешную обработку.
        //    Edge case: mark fail после handle OK — projection handlers (UPSERT)
        //    безвредны при повторе, notification handlers должны быть idempotent.
        if let Err(e) = self.inbox.mark_processed(
            envelope.event_id,
            &envelope.event_type,
            &envelope.source,
            handler_name,
        ).await {
            tracing::warn!(
                event_id = %envelope.event_id,
                handler = handler_name,
                error = %e,
                "inbox: mark_processed failed (handler already succeeded, \
                 next retry may re-process — ensure handler is idempotent)"
            );
        }

        Ok(())
    }

    fn event_type(&self) -> &'static str {
        self.inner.event_type()
    }

    fn handler_name(&self) -> &str {
        self.inner.handler_name()
    }
}
