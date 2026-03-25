//! Inbox dedup — exactly-once event processing через `common.inbox`.
//!
//! `INSERT ON CONFLICT DO NOTHING` — простейший механизм дедупликации.
//! Event handler вызывает `try_process()` первым делом:
//! - `true` → обрабатывай событие
//! - `false` → уже обработано, skip

use std::sync::Arc;

use uuid::Uuid;

use crate::pool::PgPool;

/// Guard для exactly-once обработки событий.
///
/// ```ignore
/// if !inbox.try_process(envelope.event_id, &envelope.event_type, &envelope.source).await? {
///     return Ok(()); // уже обработано
/// }
/// ```
pub struct InboxGuard {
    pool: Arc<PgPool>,
}

impl InboxGuard {
    /// Создать guard с указанным pool.
    #[must_use]
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Попытаться зарегистрировать обработку события.
    ///
    /// Возвращает `true`, если событие новое (нужно обработать).
    /// Возвращает `false`, если уже обработано (skip).
    ///
    /// # Errors
    ///
    /// `anyhow::Error` при ошибке SQL.
    pub async fn try_process(
        &self,
        event_id: Uuid,
        event_type: &str,
        source: &str,
    ) -> Result<bool, anyhow::Error> {
        let client = self.pool.get().await?;
        let rows_affected = clorinde_gen::queries::common::inbox::try_insert_inbox()
            .bind(&client, &event_id, &event_type, &source)
            .await?;
        Ok(rows_affected == 1)
    }
}
