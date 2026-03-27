//! Type erasure и реестр обработчиков событий.
//!
//! `ErasedEventHandler` — type-erased версия `EventHandler`.
//! `EventHandlerAdapter` — обёртка, связывающая типизированный handler
//! с нетипизированным `EventEnvelope`.
//! `HandlerRegistry` — реестр handler'ов по типу события.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use tokio::sync::RwLock;

use crate::envelope::EventEnvelope;
use crate::traits::EventHandler;

/// Type-erased handler. Bus работает с этим trait,
/// не зная конкретный тип события.
///
/// Реализация (`EventHandlerAdapter`) десериализует `payload`
/// из конверта в конкретный тип и вызывает типизированный `handle()`.
#[async_trait]
pub trait ErasedEventHandler: Send + Sync + 'static {
    /// Обработать конверт. Реализация десериализует payload
    /// в конкретный тип и вызывает типизированный `handle()`.
    async fn handle_envelope(&self, envelope: &EventEnvelope) -> Result<(), anyhow::Error>;

    /// Тип события, на которое подписан handler (для routing).
    fn event_type(&self) -> &'static str;

    /// Имя handler'а для inbox dedup и event registry.
    /// Default: `event_type()` (backward-compatible).
    fn handler_name(&self) -> &str {
        self.event_type()
    }
}

/// Обёртка, реализующая `ErasedEventHandler` для конкретного `EventHandler`.
///
/// Мост между типизированным миром (domain events) и нетипизированным
/// транспортным уровнем (`EventEnvelope` с `serde_json::Value` payload).
pub struct EventHandlerAdapter<H: EventHandler> {
    handler: H,
    type_name: &'static str,
}

impl<H: EventHandler> EventHandlerAdapter<H> {
    /// Обернуть типизированный handler.
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            type_name: std::any::type_name::<H>(),
        }
    }
}

#[async_trait]
impl<H> ErasedEventHandler for EventHandlerAdapter<H>
where
    H: EventHandler,
    H::Event: DeserializeOwned,
{
    async fn handle_envelope(&self, envelope: &EventEnvelope) -> Result<(), anyhow::Error> {
        let event: H::Event = envelope.deserialize_payload()?;
        self.handler.handle(&event).await
    }

    fn event_type(&self) -> &'static str {
        self.handler.handled_event_type()
    }

    fn handler_name(&self) -> &str {
        self.type_name
    }
}

/// Реестр обработчиков событий.
///
/// Хранит handler'ы, сгруппированные по типу события.
/// Потокобезопасен: `RwLock` позволяет параллельное чтение
/// при dispatch и эксклюзивную запись при регистрации.
pub struct HandlerRegistry {
    handlers: RwLock<HashMap<String, Vec<Arc<dyn ErasedEventHandler>>>>,
}

impl HandlerRegistry {
    /// Создать пустой реестр.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Зарегистрировать handler по типу события.
    pub async fn register(&self, event_type: &str, handler: Arc<dyn ErasedEventHandler>) {
        let mut map = self.handlers.write().await;
        map.entry(event_type.to_string()).or_default().push(handler);
    }

    /// Получить handler'ы для данного типа события.
    pub async fn get_handlers(&self, event_type: &str) -> Vec<Arc<dyn ErasedEventHandler>> {
        let map = self.handlers.read().await;
        map.get(event_type).cloned().unwrap_or_default()
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::DomainEvent;
    use kernel::types::{RequestContext, TenantId, UserId};
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct EventA {
        id: Uuid,
    }

    impl DomainEvent for EventA {
        fn event_type(&self) -> &'static str {
            "erp.test.event_a.v1"
        }
        fn aggregate_id(&self) -> Uuid {
            self.id
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct EventB {
        id: Uuid,
    }

    impl DomainEvent for EventB {
        fn event_type(&self) -> &'static str {
            "erp.test.event_b.v1"
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
        type Event = EventA;

        async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn handled_event_type(&self) -> &'static str {
            "erp.test.event_a.v1"
        }
    }

    struct HandlerB;

    #[async_trait]
    impl EventHandler for HandlerB {
        type Event = EventB;

        async fn handle(&self, _event: &Self::Event) -> Result<(), anyhow::Error> {
            Ok(())
        }

        fn handled_event_type(&self) -> &'static str {
            "erp.test.event_b.v1"
        }
    }

    #[tokio::test]
    async fn register_and_get_handlers() {
        let registry = HandlerRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let handler = Arc::new(EventHandlerAdapter::new(CountingHandler {
            count: counter.clone(),
        }));
        registry.register("erp.test.event_a.v1", handler).await;

        let handlers = registry.get_handlers("erp.test.event_a.v1").await;
        assert_eq!(handlers.len(), 1);
    }

    #[tokio::test]
    async fn different_event_types_route_independently() {
        let registry = HandlerRegistry::new();

        let handler_a = Arc::new(EventHandlerAdapter::new(CountingHandler {
            count: Arc::new(AtomicUsize::new(0)),
        }));
        let handler_b = Arc::new(EventHandlerAdapter::new(HandlerB));

        registry.register("erp.test.event_a.v1", handler_a).await;
        registry.register("erp.test.event_b.v1", handler_b).await;

        assert_eq!(registry.get_handlers("erp.test.event_a.v1").await.len(), 1);
        assert_eq!(registry.get_handlers("erp.test.event_b.v1").await.len(), 1);
    }

    #[tokio::test]
    async fn unknown_event_type_returns_empty() {
        let registry = HandlerRegistry::new();
        let handlers = registry.get_handlers("erp.test.nonexistent.v1").await;
        assert!(handlers.is_empty());
    }

    #[tokio::test]
    async fn handle_envelope_deserializes_and_calls() {
        let counter = Arc::new(AtomicUsize::new(0));
        let adapter = EventHandlerAdapter::new(CountingHandler {
            count: counter.clone(),
        });

        let event = EventA { id: Uuid::now_v7() };
        let ctx = RequestContext::new(TenantId::new(), UserId::new());
        let envelope = EventEnvelope::from_domain_event(&event, &ctx, "test").unwrap();

        adapter.handle_envelope(&envelope).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
