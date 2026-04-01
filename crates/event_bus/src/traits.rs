//! Трейты шины событий и обработчиков.
//!
//! `EventHandler` — типизированный обработчик конкретного события.
//! `EventBus` — абстракция шины (заменяема на `RabbitMQ`/NATS/Kafka).

use std::sync::Arc;

use async_trait::async_trait;
use kernel::DomainEvent;

use crate::envelope::EventEnvelope;
use crate::registry::ErasedEventHandler;

/// Обработчик событий. Реализуется подписчиками.
///
/// Пример: Finance реализует `EventHandler` для `GoodsShipped`,
/// чтобы создать GL-проводку при отгрузке со склада.
///
/// Bounds `Send + Sync + 'static` необходимы: handler'ы хранятся
/// в `Arc` и вызываются из разных потоков tokio runtime.
#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    /// Тип события, на которое подписан handler.
    /// Определяет routing: bus доставляет только события нужного типа.
    type Event: DomainEvent;

    /// Обработать событие.
    ///
    /// Может быть async (обращение к БД, вызов сервисов).
    /// Ошибка попадёт в retry/DLQ (Layer 3b).
    async fn handle(&self, event: &Self::Event) -> Result<(), anyhow::Error>;

    /// Тип события как строка для routing.
    /// По умолчанию создаёт временный экземпляр — переопределите,
    /// если конструирование `Event` дорогое.
    fn handled_event_type(&self) -> &'static str;
}

/// Шина событий. Центральный компонент межмодульного взаимодействия.
///
/// Реализации:
/// - `InProcessBus`: tokio channels (modular monolith)
/// - В будущем: `RabbitMqBus`, `NatsBus`, `KafkaBus` (микросервисы)
///
/// Domain и Application слои зависят от этого trait,
/// не от конкретной реализации.
#[async_trait]
pub trait EventBus: Send + Sync + 'static {
    /// Опубликовать событие. Fire-and-forget.
    ///
    /// Handler'ы вызываются async, издатель не ждёт завершения.
    /// Ошибки handler'ов логируются, но не возвращаются.
    async fn publish(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error>;

    /// Опубликовать и дождаться обработки всеми handler'ами.
    ///
    /// Используется для domain events внутри TX.
    /// Возвращает первую ошибку, если хотя бы один handler упал.
    async fn publish_and_wait(&self, envelope: EventEnvelope) -> Result<(), anyhow::Error>;

    /// Зарегистрировать обработчик. Вызывается при старте приложения.
    async fn subscribe(&self, event_type: &'static str, handler: Arc<dyn ErasedEventHandler>);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Проверяем object safety: Arc<dyn EventBus> должен компилироваться
    fn _assert_event_bus_object_safe(_bus: Arc<dyn EventBus>) {}

    // Проверяем что трейты имеют правильные bounds
    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn traits_compile_with_correct_bounds() {
        // If this compiles, bounds are correct
        assert_send_sync::<Arc<dyn EventBus>>();
    }
}
