//! Outbox Relay — фоновый worker, публикующий события из outbox в `EventBus`.
//!
//! Цикл: poll неопубликованных → десериализация → `bus.publish()` → mark published.
//! При ошибке handler'а — `retry_count++`. При `retry_count >= MAX_RETRIES` — skip.

use std::sync::Arc;
use std::time::Duration;

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

        let rows = client
            .query(
                "SELECT id, tenant_id, event_id, event_type, source, payload, \
                        correlation_id, causation_id, user_id, created_at, retry_count \
                 FROM common.outbox \
                 WHERE published = false AND retry_count < $1 \
                 ORDER BY id \
                 LIMIT $2 \
                 FOR UPDATE SKIP LOCKED",
                &[&MAX_RETRIES, &self.batch_size],
            )
            .await?;

        let mut published_count = 0;

        for row in &rows {
            let id: i64 = row.get(0);
            let tenant_id_uuid: uuid::Uuid = row.get(1);
            let event_id: uuid::Uuid = row.get(2);
            let event_type: String = row.get(3);
            let source: String = row.get(4);
            let payload: serde_json::Value = row.get(5);
            let correlation_id: uuid::Uuid = row.get(6);
            let causation_id: uuid::Uuid = row.get(7);
            let user_id_uuid: uuid::Uuid = row.get(8);
            let created_at: chrono::DateTime<chrono::Utc> = row.get(9);

            let envelope = EventEnvelope {
                event_id,
                event_type,
                source,
                tenant_id: TenantId::from_uuid(tenant_id_uuid),
                correlation_id,
                causation_id,
                user_id: UserId::from_uuid(user_id_uuid),
                timestamp: created_at,
                payload,
            };

            match self.bus.publish(envelope).await {
                Ok(()) => {
                    client
                        .execute(
                            "UPDATE common.outbox SET published = true, published_at = now() \
                             WHERE id = $1",
                            &[&id],
                        )
                        .await?;
                    published_count += 1;
                }
                Err(e) => {
                    warn!(
                        event_id = %event_id,
                        error = %e,
                        "outbox publish failed, incrementing retry"
                    );
                    client
                        .execute(
                            "UPDATE common.outbox SET retry_count = retry_count + 1 WHERE id = $1",
                            &[&id],
                        )
                        .await?;
                }
            }
        }

        client.batch_execute("COMMIT").await?;

        Ok(published_count)
    }
}
