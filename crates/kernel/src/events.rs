//! Domain events и `CloudEvent` envelope.
//!
//! `DomainEvent` — факт, произошедший внутри агрегата (неизменяем).
//! `CloudEvent<T>` — стандартный конверт (CNCF `CloudEvents` v1.0)
//! для integration events, пересекающих границы BC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{RequestContext, TenantId, UserId};

/// Контракт для доменных событий.
///
/// Каждое событие — неизменяемый факт, произошедший в результате
/// выполнения команды. Сериализуется для event store и outbox.
pub trait DomainEvent: Serialize + Send + Sync + 'static {
    /// Тип события для routing и event store.
    ///
    /// Формат: `"erp.bc_name.event_name.v1"`, например
    /// `"erp.warehouse.goods_received.v1"`.
    fn event_type(&self) -> &'static str;

    /// ID агрегата, породившего событие.
    fn aggregate_id(&self) -> Uuid;
}

/// Integration event envelope по стандарту CNCF `CloudEvents` v1.0.
///
/// `data` содержит payload с примитивными типами (String, Uuid, числа) —
/// это Anti-Corruption Layer между BC. Каждый BC сам валидирует
/// входящие данные по своим правилам.
///
/// Bound `T: Serialize` вынесен на impl-блоки, чтобы `#[derive(Deserialize)]`
/// не требовал `T: Serialize` при десериализации.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudEvent<T> {
    /// Версия спецификации `CloudEvents` (всегда "1.0").
    pub specversion: String,
    /// Уникальный идентификатор события (UUID v7).
    pub id: Uuid,
    /// Источник события (имя BC): "warehouse", "finance".
    pub source: String,
    /// Тип события: `"erp.warehouse.goods_received.v1"`.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Время создания события.
    pub time: DateTime<Utc>,
    /// MIME-тип содержимого (всегда "application/json").
    pub datacontenttype: String,
    /// ID агрегата как строка (опционально).
    pub subject: Option<String>,

    // ERP-расширения
    /// Tenant, в контексте которого произошло событие.
    pub tenant_id: TenantId,
    /// Сквозной ID трассировки.
    pub correlation_id: Uuid,
    /// ID команды/события-причины.
    pub causation_id: Uuid,
    /// Пользователь, инициировавший цепочку.
    pub user_id: UserId,

    /// Payload — примитивные типы, не доменные value objects.
    pub data: T,
}

impl<T: Serialize> CloudEvent<T> {
    /// Создать `CloudEvent` из payload и контекста запроса.
    ///
    /// Автоматически заполняет specversion, id (UUID v7), time, datacontenttype.
    #[must_use]
    pub fn new(
        source: &str,
        event_type: &str,
        subject: Option<String>,
        data: T,
        ctx: &RequestContext,
    ) -> Self {
        Self {
            specversion: "1.0".to_string(),
            id: Uuid::now_v7(),
            source: source.to_string(),
            event_type: event_type.to_string(),
            time: Utc::now(),
            datacontenttype: "application/json".to_string(),
            subject,
            tenant_id: ctx.tenant_id,
            correlation_id: ctx.correlation_id,
            causation_id: ctx.causation_id,
            user_id: ctx.user_id,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TenantId, UserId};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestPayload {
        sku: String,
        quantity: i32,
    }

    #[derive(Debug, Clone, Serialize)]
    struct TestEvent {
        aggregate: Uuid,
        value: String,
    }

    impl DomainEvent for TestEvent {
        fn event_type(&self) -> &'static str {
            "erp.test.something_happened.v1"
        }

        fn aggregate_id(&self) -> Uuid {
            self.aggregate
        }
    }

    #[test]
    fn domain_event_trait_methods() {
        let agg_id = Uuid::now_v7();
        let event = TestEvent {
            aggregate: agg_id,
            value: "test".to_string(),
        };
        assert_eq!(event.event_type(), "erp.test.something_happened.v1");
        assert_eq!(event.aggregate_id(), agg_id);
    }

    #[test]
    fn cloud_event_new_fills_standard_fields() {
        let ctx = RequestContext::new(TenantId::new(), UserId::new());
        let payload = TestPayload {
            sku: "SKU-001".to_string(),
            quantity: 10,
        };

        let event = CloudEvent::new(
            "warehouse",
            "erp.warehouse.goods_received.v1",
            Some("agg-123".to_string()),
            payload,
            &ctx,
        );

        assert_eq!(event.specversion, "1.0");
        assert_eq!(event.id.get_version_num(), 7);
        assert_eq!(event.source, "warehouse");
        assert_eq!(event.event_type, "erp.warehouse.goods_received.v1");
        assert_eq!(event.datacontenttype, "application/json");
        assert_eq!(event.subject, Some("agg-123".to_string()));
        assert_eq!(event.tenant_id, ctx.tenant_id);
        assert_eq!(event.correlation_id, ctx.correlation_id);
        assert_eq!(event.causation_id, ctx.causation_id);
        assert_eq!(event.user_id, ctx.user_id);

        // time is recent
        let elapsed = Utc::now() - event.time;
        assert!(elapsed.num_seconds() < 1);
    }

    #[test]
    fn cloud_event_serde_round_trip() {
        let ctx = RequestContext::new(TenantId::new(), UserId::new());
        let payload = TestPayload {
            sku: "SKU-002".to_string(),
            quantity: 5,
        };

        let event = CloudEvent::new(
            "warehouse",
            "erp.warehouse.goods_shipped.v1",
            None,
            payload,
            &ctx,
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: CloudEvent<TestPayload> = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.specversion, "1.0");
        assert_eq!(deserialized.id, event.id);
        assert_eq!(deserialized.source, "warehouse");
        assert_eq!(deserialized.data.sku, "SKU-002");
        assert_eq!(deserialized.data.quantity, 5);
        assert_eq!(deserialized.tenant_id, event.tenant_id);
    }
}
