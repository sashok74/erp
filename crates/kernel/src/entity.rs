//! Контракты для сущностей и агрегатов.
//!
//! `Entity` — базовый trait для всех доменных сущностей.
//! `AggregateRoot` — корень консистентности с поддержкой Event Sourcing.

use crate::events::DomainEvent;
use crate::types::EntityId;

/// Базовый контракт для доменных сущностей.
///
/// Каждая сущность имеет уникальный идентификатор.
pub trait Entity {
    /// Идентификатор сущности.
    fn id(&self) -> EntityId;
}

/// Корень агрегата — главная единица консистентности.
///
/// Поддерживает два режима работы (гибридный ES из ADR v1):
///
/// - **Полный Event Sourcing** (Warehouse, Finance): состояние восстанавливается
///   из цепочки событий через `apply()`. Команды генерируют события,
///   `take_events()` забирает их для сохранения в event store.
///
/// - **CRUD + events** (остальные BC): состояние в обычных таблицах,
///   но изменения генерируют события для шины.
pub trait AggregateRoot: Send + Sync {
    /// Тип событий этого агрегата.
    type Event: DomainEvent;

    /// Применить событие к состоянию.
    ///
    /// Для ES-агрегатов — обновляет поля из события (replay).
    /// Для CRUD+events — аналогично, но состояние также хранится в таблице.
    fn apply(&mut self, event: &Self::Event);

    /// Забрать накопленные неопубликованные события.
    ///
    /// После вызова внутренний буфер пуст. Использует `std::mem::take`
    /// для перемещения без копирования.
    fn take_events(&mut self) -> Vec<Self::Event>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use uuid::Uuid;

    // --- Тестовый агрегат Counter с событием Incremented ---

    #[derive(Debug, Clone, Serialize)]
    struct Incremented {
        value: i32,
        aggregate: Uuid,
    }

    impl DomainEvent for Incremented {
        fn event_type(&self) -> &'static str {
            "erp.test.incremented.v1"
        }

        fn aggregate_id(&self) -> Uuid {
            self.aggregate
        }
    }

    struct Counter {
        id: EntityId,
        total: i32,
        events: Vec<Incremented>,
    }

    impl Counter {
        fn new() -> Self {
            Self {
                id: EntityId::new(),
                total: 0,
                events: Vec::new(),
            }
        }

        fn increment(&mut self, value: i32) {
            let event = Incremented {
                value,
                aggregate: *self.id.as_uuid(),
            };
            self.apply(&event);
            self.events.push(event);
        }
    }

    impl Entity for Counter {
        fn id(&self) -> EntityId {
            self.id
        }
    }

    impl AggregateRoot for Counter {
        type Event = Incremented;

        fn apply(&mut self, event: &Self::Event) {
            self.total += event.value;
        }

        fn take_events(&mut self) -> Vec<Self::Event> {
            std::mem::take(&mut self.events)
        }
    }

    #[test]
    fn counter_apply_updates_state() {
        let mut counter = Counter::new();
        counter.increment(5);
        assert_eq!(counter.total, 5);

        counter.increment(3);
        assert_eq!(counter.total, 8);
    }

    #[test]
    fn counter_take_events_returns_and_clears() {
        let mut counter = Counter::new();
        counter.increment(5);
        counter.increment(3);

        let events = counter.take_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].value, 5);
        assert_eq!(events[1].value, 3);

        // Buffer is empty after take
        let events_again = counter.take_events();
        assert!(events_again.is_empty());
    }

    #[test]
    fn counter_entity_id() {
        let counter = Counter::new();
        let id = counter.id();
        assert_eq!(id.as_uuid().get_version_num(), 7);
    }
}
