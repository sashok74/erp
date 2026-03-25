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
use tracing::{debug, error, warn};

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
}

impl OutboxRelay {
    /// Создать relay с указанными параметрами.
    #[must_use]
    pub fn new(
        pool: Arc<PgPool>,
        bus: Arc<dyn EventBus>,
        poll_interval: Duration,
        batch_size: i64,
    ) -> Self {
        Self {
            pool,
            bus,
            poll_interval,
            batch_size,
        }
    }

    /// Бесконечный loop: poll → publish → sleep. Вызывать через `tokio::spawn`.
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
            tokio::time::sleep(self.poll_interval).await;
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
            // Skip entries that have exceeded max retries (clorinde query
            // does not filter by retry_count, so we check here).
            if row.retry_count >= MAX_RETRIES {
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

            match self.bus.publish(envelope).await {
                Ok(()) => {
                    clorinde_gen::queries::common::outbox::mark_published()
                        .bind(&client, &row.id)
                        .await?;
                    published_count += 1;
                }
                Err(e) => {
                    warn!(
                        event_id = %row.event_id,
                        error = %e,
                        "outbox publish failed, incrementing retry"
                    );
                    clorinde_gen::queries::common::outbox::increment_retry()
                        .bind(&client, &row.id)
                        .await?;
                }
            }
        }

        client.batch_execute("COMMIT").await?;

        Ok(published_count)
    }
}
