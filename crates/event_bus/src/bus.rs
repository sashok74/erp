//! In-process реализация `EventBus` на tokio.
//!
//! Все BC в одном процессе — события передаются через память.
//! При переходе к микросервисам заменяется на `RabbitMqBus`/`NatsBus`,
//! реализующий тот же trait `EventBus`.

use std::sync::Arc;

use async_trait::async_trait;

use crate::envelope::EventEnvelope;
use crate::registry::{ErasedEventHandler, HandlerRegistry};
use crate::traits::EventBus;

/// In-process `EventBus` для modular monolith.
///
/// Handler'ы регистрируются через `subscribe()`, события доставляются
/// напрямую через вызовы в памяти.
///
/// - `publish()` — fire-and-forget: handler'ы запускаются в `tokio::spawn`,
///   ошибки логируются, но не возвращаются издателю.
/// - `publish_and_wait()` — синхронный dispatch: ждёт завершения
///   всех handler'ов, возвращает первую ошибку.
pub struct InProcessBus {
    registry: HandlerRegistry,
}

impl InProcessBus {
    /// Создать новый bus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: HandlerRegistry::new(),
        }
    }

    /// Получить ссылку на registry для прямой регистрации handler'ов.
    #[must_use]
    pub fn registry(&self) -> &HandlerRegistry {
        &self.registry
    }
}

impl Default for InProcessBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for InProcessBus {
    async fn publish(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error> {
        let handlers = self.registry.get_handlers(&envelope.event_type).await;

        for handler in handlers {
            let env = envelope.clone();
            tokio::spawn(async move {
                if let Err(e) = handler.handle_envelope(&env).await {
                    tracing::warn!(
                        event_type = %env.event_type,
                        event_id = %env.event_id,
                        error = %e,
                        "Event handler failed (fire-and-forget)"
                    );
                }
            });
        }

        Ok(())
    }

    async fn publish_and_wait(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error> {
        let handlers = self.registry.get_handlers(&envelope.event_type).await;

        for handler in handlers {
            handler.handle_envelope(&envelope).await?;
        }

        Ok(())
    }

    async fn subscribe(&self, event_type: &'static str, handler: Arc<dyn ErasedEventHandler>) {
        self.registry.register(event_type, handler).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::EventHandlerAdapter;
    use crate::traits::EventHandler;
    use kernel::DomainEvent;
    use kernel::types::{RequestContext, TenantId, UserId};
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestEvent {
        id: Uuid,
    }

    impl DomainEvent for TestEvent {
        fn event_type(&self) -> &'static str {
            "erp.test.bus_event.v1"
        }
        fn aggregate_id(&self) -> Uuid {
            self.id
        }
    }

    struct CountingHandler {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EventHandler for CountingHandler {
        type Event = TestEvent;

        async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn handled_event_type(&self) -> &'static str {
            "erp.test.bus_event.v1"
        }
    }

    struct FailingHandler;

    #[async_trait]
    impl EventHandler for FailingHandler {
        type Event = TestEvent;

        async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
            anyhow::bail!("handler error")
        }

        fn handled_event_type(&self) -> &'static str {
            "erp.test.bus_event.v1"
        }
    }

    fn make_envelope() -> EventEnvelope {
        let event = TestEvent { id: Uuid::now_v7() };
        let ctx = RequestContext::new(TenantId::new(), UserId::new());
        EventEnvelope::from_domain_event(&event, &ctx, "test").unwrap()
    }

    #[tokio::test]
    async fn subscribe_and_publish_calls_handler() {
        let bus = InProcessBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let handler = Arc::new(EventHandlerAdapter::new(CountingHandler {
            count: counter.clone(),
        }));
        bus.subscribe("erp.test.bus_event.v1", handler).await;

        bus.publish(make_envelope()).await.unwrap();

        // Give spawned task time to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn two_handlers_on_same_type_both_called() {
        let bus = InProcessBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let h1 = Arc::new(EventHandlerAdapter::new(CountingHandler {
            count: counter.clone(),
        }));
        let h2 = Arc::new(EventHandlerAdapter::new(CountingHandler {
            count: counter.clone(),
        }));
        bus.subscribe("erp.test.bus_event.v1", h1).await;
        bus.subscribe("erp.test.bus_event.v1", h2).await;

        bus.publish_and_wait(make_envelope()).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn publish_unknown_event_type_is_ok() {
        let bus = InProcessBus::new();

        // No handlers registered — publish should succeed silently
        let result = bus.publish(make_envelope()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn publish_and_wait_returns_handler_error() {
        let bus = InProcessBus::new();

        let handler = Arc::new(EventHandlerAdapter::new(FailingHandler));
        bus.subscribe("erp.test.bus_event.v1", handler).await;

        let result = bus.publish_and_wait(make_envelope()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("handler error"));
    }

    #[tokio::test]
    async fn publish_fire_and_forget_ignores_handler_error() {
        let bus = InProcessBus::new();

        let handler = Arc::new(EventHandlerAdapter::new(FailingHandler));
        bus.subscribe("erp.test.bus_event.v1", handler).await;

        // publish (fire-and-forget) should return Ok even if handler fails
        let result = bus.publish(make_envelope()).await;
        assert!(result.is_ok());
    }
}
