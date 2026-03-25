//! Доменные события Warehouse BC.

use bigdecimal::BigDecimal;
use kernel::DomainEvent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Товар принят на склад.
///
/// Payload содержит примитивные типы (Anti-Corruption Layer),
/// не доменные value objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoodsReceived {
    pub item_id: Uuid,
    pub sku: String,
    pub quantity: BigDecimal,
    pub new_balance: BigDecimal,
    pub doc_number: String,
}

impl DomainEvent for GoodsReceived {
    fn event_type(&self) -> &'static str {
        "erp.warehouse.goods_received.v1"
    }

    fn aggregate_id(&self) -> Uuid {
        self.item_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goods_received_event_type() {
        let event = GoodsReceived {
            item_id: Uuid::now_v7(),
            sku: "BOLT-42".into(),
            quantity: BigDecimal::from(100),
            new_balance: BigDecimal::from(100),
            doc_number: "ПРХ-000001".into(),
        };
        assert_eq!(event.event_type(), "erp.warehouse.goods_received.v1");
    }

    #[test]
    fn goods_received_aggregate_id() {
        let id = Uuid::now_v7();
        let event = GoodsReceived {
            item_id: id,
            sku: "BOLT-42".into(),
            quantity: BigDecimal::from(100),
            new_balance: BigDecimal::from(100),
            doc_number: "ПРХ-000001".into(),
        };
        assert_eq!(event.aggregate_id(), id);
    }

    #[test]
    fn goods_received_serde_round_trip() {
        let event = GoodsReceived {
            item_id: Uuid::now_v7(),
            sku: "NUT-7".into(),
            quantity: BigDecimal::from(50),
            new_balance: BigDecimal::from(150),
            doc_number: "ПРХ-000002".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: GoodsReceived = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.sku, "NUT-7");
    }
}
