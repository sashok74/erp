//! Outbox Relay — фоновый worker, публикующий события из outbox в `EventBus`.
//!
//! Цикл: poll неопубликованных → десериализация → `bus.publish()` → mark published.
//! При ошибке handler'а — `retry_count++`. При `retry_count >= MAX_RETRIES` — skip.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use event_bus::EventEnvelope;
use event_bus::traits::EventBus;
use kernel::types::{TenantId, UserId};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::pool::PgPool;

/// Максимальное количество попыток публикации.
const MAX_RETRIES: i32 = 3;

/// Outbox Relay — background tokio task.
///
/// Запускается через `tokio::spawn(relay.run())` в gateway `main()`.
/// Для тестов — вызывать `poll_and_publish()` напрямую.
pub struct OutboxRelay {
    pool: Arc<PgPool>,
    bus: Arc<dyn EventBus>,
    poll_interval: Duration,
    batch_size: i64,
    cancel: CancellationToken,
}

impl OutboxRelay {
    /// Создать relay с указанными параметрами.
    #[must_use]
    pub fn new(
        pool: Arc<PgPool>,
        bus: Arc<dyn EventBus>,
        poll_interval: Duration,
        batch_size: i64,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            pool,
            bus,
            poll_interval,
            batch_size,
            cancel,
        }
    }

    /// Цикл: poll → publish → sleep. Завершается при отмене `CancellationToken`.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку только при невосстановимых проблемах с pool.
    pub async fn run(&self) -> Result<(), anyhow::Error> {
        loop {
            match self.poll_and_publish().await {
                Ok(count) => {
                    if count > 0 {
                        debug!(published = count, "outbox relay cycle");
                    }
                }
                Err(e) => {
                    error!(error = %e, "outbox relay poll failed");
                }
            }
            tokio::select! {
                () = tokio::time::sleep(self.poll_interval) => {}
                () = self.cancel.cancelled() => {
                    info!("outbox relay shutting down");
                    return Ok(());
                }
            }
        }
    }

    /// Одна итерация: poll batch → publish каждое → mark/retry.
    /// Возвращает количество успешно опубликованных событий.
    ///
    /// # Errors
    ///
    /// `anyhow::Error` при ошибке pool/SQL.
    pub async fn poll_and_publish(&self) -> Result<usize, anyhow::Error> {
        let client = self.pool.get().await?;

        // BEGIN — FOR UPDATE SKIP LOCKED требует транзакцию.
        client.batch_execute("BEGIN").await?;

        let rows = clorinde_gen::queries::common::outbox::get_unpublished_events()
            .bind(&client, &self.batch_size)
            .all()
            .await?;

        let mut published_count = 0;

        for row in &rows {
            // Events that exceeded max retries → move to dead letter queue.
            if row.retry_count >= MAX_RETRIES {
                self.move_to_dlq(&client, row, "max retries exceeded")
                    .await?;
                continue;
            }

            let envelope = EventEnvelope {
                event_id: row.event_id,
                event_type: row.event_type.clone(),
                source: row.source.clone(),
                tenant_id: TenantId::from_uuid(row.tenant_id),
                correlation_id: row.correlation_id,
                causation_id: row.causation_id,
                user_id: UserId::from_uuid(row.user_id),
                timestamp: row.created_at.with_timezone(&Utc),
                payload: row.payload.clone(),
            };

            match self.bus.publish_and_wait(envelope).await {
                Ok(()) => {
                    clorinde_gen::queries::common::outbox::mark_published()
                        .bind(&client, &row.id)
                        .await?;
                    published_count += 1;
                }
                Err(e) => {
                    let new_retry = row.retry_count + 1;
                    if new_retry >= MAX_RETRIES {
                        self.move_to_dlq(&client, row, &e.to_string()).await?;
                    } else {
                        warn!(
                            event_id = %row.event_id,
                            retry = new_retry,
                            error = %e,
                            "outbox publish failed, incrementing retry"
                        );
                        clorinde_gen::queries::common::outbox::increment_retry()
                            .bind(&client, &row.id)
                            .await?;
                    }
                }
            }
        }

        client.batch_execute("COMMIT").await?;

        Ok(published_count)
    }

    /// Перенести событие в dead letter queue и пометить в outbox как обработанное.
    async fn move_to_dlq(
        &self,
        client: &impl clorinde_gen::client::GenericClient,
        row: &clorinde_gen::queries::common::outbox::GetUnpublishedEvents,
        last_error: &str,
    ) -> Result<(), anyhow::Error> {
        error!(
            event_id = %row.event_id,
            event_type = %row.event_type,
            retry_count = row.retry_count,
            "event moved to dead letter queue"
        );
        clorinde_gen::queries::common::outbox::move_to_dlq()
            .bind(client, &last_error, &row.id)
            .await?;
        clorinde_gen::queries::common::outbox::mark_dlq()
            .bind(client, &row.id)
            .await?;
        Ok(())
    }
}
