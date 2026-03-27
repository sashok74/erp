//! `InboxBusDecorator` — decorator над `EventBus` для enforced consumer dedup.
//!
//! `subscribe()` оборачивает каждый handler в `InboxAwareHandler`.
//! Собирает event registry для `/dev/events`.
//! `publish`/`publish_and_wait` — passthrough к inner bus.

use std::sync::Arc;

use async_trait::async_trait;
use event_bus::registry::ErasedEventHandler;
use event_bus::traits::EventBus;
use event_bus::EventEnvelope;
use tokio::sync::RwLock;

use crate::inbox::{InboxAwareHandler, InboxGuard};
use crate::pool::PgPool;

/// Запись реестра: один handler, подписанный на `event_type`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EventRegistryEntry {
    pub event_type: String,
    pub handler_name: String,
}

/// Decorator над `EventBus`:
/// 1. `subscribe()` → оборачивает handler в `InboxAwareHandler`
/// 2. собирает event registry для `/dev/events`
/// 3. `publish`/`publish_and_wait` → passthrough к inner bus
pub struct InboxBusDecorator {
    inner: Arc<dyn EventBus>,
    inbox: Arc<InboxGuard>,
    registry: RwLock<Vec<EventRegistryEntry>>,
}

impl InboxBusDecorator {
    pub fn new(inner: Arc<dyn EventBus>, pool: Arc<PgPool>) -> Self {
        Self {
            inner,
            inbox: Arc::new(InboxGuard::new(pool)),
            registry: RwLock::new(Vec::new()),
        }
    }

    /// Карта зарегистрированных событий → handler'ов.
    pub async fn event_map(&self) -> Vec<EventRegistryEntry> {
        self.registry.read().await.clone()
    }
}

#[async_trait]
impl EventBus for InboxBusDecorator {
    async fn subscribe(&self, event_type: &'static str, handler: Arc<dyn ErasedEventHandler>) {
        let entry = EventRegistryEntry {
            event_type: event_type.to_string(),
            handler_name: handler.handler_name().to_string(),
        };
        self.registry.write().await.push(entry);

        let wrapped = Arc::new(InboxAwareHandler::new(handler, self.inbox.clone()));
        self.inner.subscribe(event_type, wrapped).await;
    }

    async fn publish(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error> {
        self.inner.publish(envelope).await
    }

    async fn publish_and_wait(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error> {
        self.inner.publish_and_wait(envelope).await
    }
}
