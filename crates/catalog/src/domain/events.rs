//! Доменные события Catalog BC.

use kernel::DomainEvent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Товар создан в каталоге.
///
/// Payload содержит примитивные типы (Anti-Corruption Layer),
/// не доменные value objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCreated {
    pub tenant_id: Uuid,
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

impl DomainEvent for ProductCreated {
    fn event_type(&self) -> &'static str {
        "erp.catalog.product_created.v1"
    }

    fn aggregate_id(&self) -> Uuid {
        self.product_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_created_event_type() {
        let event = ProductCreated {
            tenant_id: Uuid::now_v7(),
            product_id: Uuid::now_v7(),
            sku: "BOLT-42".into(),
            name: "Болт M10x50".into(),
            category: "Крепёж".into(),
            unit: "шт".into(),
        };
        assert_eq!(event.event_type(), "erp.catalog.product_created.v1");
    }

    #[test]
    fn product_created_aggregate_id() {
        let id = Uuid::now_v7();
        let event = ProductCreated {
            tenant_id: Uuid::now_v7(),
            product_id: id,
            sku: "BOLT-42".into(),
            name: "Болт M10x50".into(),
            category: "Крепёж".into(),
            unit: "шт".into(),
        };
        assert_eq!(event.aggregate_id(), id);
    }

    #[test]
    fn product_created_serde_round_trip() {
        let event = ProductCreated {
            tenant_id: Uuid::now_v7(),
            product_id: Uuid::now_v7(),
            sku: "NUT-7".into(),
            name: "Гайка M10".into(),
            category: "Крепёж".into(),
            unit: "шт".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: ProductCreated = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.sku, "NUT-7");
        assert_eq!(restored.name, "Гайка M10");
    }
}
