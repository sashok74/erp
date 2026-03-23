//! Конверт события для транспортировки через `EventBus`.
//!
//! Payload сериализован в `serde_json::Value` — это позволяет:
//! - Передавать через in-process bus (текущая реализация)
//! - Записывать в outbox таблицу (Layer 3b)
//! - Отправлять в `RabbitMQ`/NATS/Kafka (будущее)
//! - Сохранять в event store (Layer 6)

use chrono::{DateTime, Utc};
use kernel::DomainEvent;
use kernel::types::{RequestContext, TenantId, UserId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Транспортная обёртка события.
///
/// Bus работает с конвертами, а не с типизированными событиями.
/// Handler при получении десериализует `payload` в свой конкретный тип.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Уникальный ID события (UUID v7).
    pub event_id: Uuid,
    /// Тип события для routing: `"erp.warehouse.goods_shipped.v1"`.
    pub event_type: String,
    /// Источник (имя BC): `"warehouse"`, `"finance"`.
    pub source: String,
    /// Tenant, в контексте которого произошло событие.
    pub tenant_id: TenantId,
    /// Сквозной ID трассировки.
    pub correlation_id: Uuid,
    /// ID команды/события-причины.
    pub causation_id: Uuid,
    /// Пользователь, инициировавший цепочку.
    pub user_id: UserId,
    /// Время создания конверта.
    pub timestamp: DateTime<Utc>,
    /// Сериализованное событие (type-erased).
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    /// Создать конверт из доменного события и контекста запроса.
    ///
    /// Сериализует событие в `serde_json::Value` для type erasure.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если событие не сериализуется в JSON.
    pub fn from_domain_event<E: DomainEvent>(
        event: &E,
        ctx: &RequestContext,
        source: &str,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            event_id: Uuid::now_v7(),
            event_type: event.event_type().to_string(),
            source: source.to_string(),
            tenant_id: ctx.tenant_id,
            correlation_id: ctx.correlation_id,
            causation_id: ctx.causation_id,
            user_id: ctx.user_id,
            timestamp: Utc::now(),
            payload: serde_json::to_value(event)?,
        })
    }

    /// Десериализовать payload в конкретный тип события.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку, если payload не соответствует типу `E`.
    pub fn deserialize_payload<E: DeserializeOwned>(&self) -> Result<E, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::types::{TenantId, UserId};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestEvent {
        aggregate: Uuid,
        item: String,
        quantity: i32,
    }

    impl DomainEvent for TestEvent {
        fn event_type(&self) -> &'static str {
            "erp.test.item_added.v1"
        }

        fn aggregate_id(&self) -> Uuid {
            self.aggregate
        }
    }

    #[test]
    fn from_domain_event_deserialize_round_trip() {
        let agg_id = Uuid::now_v7();
        let event = TestEvent {
            aggregate: agg_id,
            item: "BOLT-42".to_string(),
            quantity: 100,
        };
        let ctx = RequestContext::new(TenantId::new(), UserId::new());

        let envelope = EventEnvelope::from_domain_event(&event, &ctx, "warehouse").unwrap();
        let restored: TestEvent = envelope.deserialize_payload().unwrap();

        assert_eq!(restored, event);
    }

    #[test]
    fn from_domain_event_fills_metadata() {
        let event = TestEvent {
            aggregate: Uuid::now_v7(),
            item: "NUT-7".to_string(),
            quantity: 50,
        };
        let ctx = RequestContext::new(TenantId::new(), UserId::new());

        let envelope = EventEnvelope::from_domain_event(&event, &ctx, "warehouse").unwrap();

        assert_eq!(envelope.event_type, "erp.test.item_added.v1");
        assert_eq!(envelope.source, "warehouse");
        assert_eq!(envelope.tenant_id, ctx.tenant_id);
        assert_eq!(envelope.user_id, ctx.user_id);
        assert_eq!(envelope.correlation_id, ctx.correlation_id);
        assert_eq!(envelope.causation_id, ctx.causation_id);
        assert_eq!(envelope.event_id.get_version_num(), 7);
    }

    #[test]
    fn envelope_serde_round_trip() {
        let event = TestEvent {
            aggregate: Uuid::now_v7(),
            item: "WASHER-3".to_string(),
            quantity: 200,
        };
        let ctx = RequestContext::new(TenantId::new(), UserId::new());

        let envelope = EventEnvelope::from_domain_event(&event, &ctx, "warehouse").unwrap();
        let json = serde_json::to_string(&envelope).unwrap();
        let restored: EventEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.event_id, envelope.event_id);
        assert_eq!(restored.event_type, envelope.event_type);
        assert_eq!(restored.source, envelope.source);
        assert_eq!(restored.tenant_id, envelope.tenant_id);

        let event_back: TestEvent = restored.deserialize_payload().unwrap();
        assert_eq!(event_back, event);
    }
}
