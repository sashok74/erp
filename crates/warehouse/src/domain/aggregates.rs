//! Агрегат `InventoryItem` — корень консистентности для складских остатков.

use kernel::entity::{AggregateRoot, Entity};
use kernel::types::EntityId;

use super::errors::WarehouseDomainError;
use super::events::GoodsReceived;
use super::value_objects::{Quantity, Sku};

/// Товарная позиция на складе.
///
/// Единственный инвариант: `balance >= 0`.
pub struct InventoryItem {
    id: EntityId,
    sku: Sku,
    balance: Quantity,
    pending_events: Vec<GoodsReceived>,
}

impl InventoryItem {
    /// Создать новую позицию с нулевым балансом.
    #[must_use]
    pub fn new(id: EntityId, sku: Sku) -> Self {
        Self {
            id,
            sku,
            balance: Quantity::zero(),
            pending_events: Vec::new(),
        }
    }

    /// Восстановить агрегат из БД (без pending events).
    #[must_use]
    pub fn from_state(id: EntityId, sku: Sku, balance: Quantity) -> Self {
        Self {
            id,
            sku,
            balance,
            pending_events: Vec::new(),
        }
    }

    /// Принять товар на склад.
    ///
    /// Увеличивает баланс, создаёт `GoodsReceived` event.
    /// Возвращает ссылку на созданное событие.
    ///
    /// # Errors
    ///
    /// `ZeroQuantity` — если `qty` == 0.
    ///
    /// # Panics
    ///
    /// Не паникует: `expect` на `.last()` гарантирован предшествующим `push`.
    pub fn receive(
        &mut self,
        qty: &Quantity,
        doc_number: String,
    ) -> Result<&GoodsReceived, WarehouseDomainError> {
        if qty.is_zero() {
            return Err(WarehouseDomainError::ZeroQuantity);
        }

        let new_balance = self.balance.clone() + qty.clone();

        let event = GoodsReceived {
            item_id: *self.id.as_uuid(),
            sku: self.sku.as_str().to_string(),
            quantity: qty.value().clone(),
            new_balance: new_balance.value().clone(),
            doc_number,
        };

        self.apply(&event);
        self.pending_events.push(event);
        // SAFETY: we just pushed an event, so `last()` is always `Some`.
        Ok(self.pending_events.last().expect("just pushed"))
    }

    /// Текущий баланс.
    #[must_use]
    pub fn balance(&self) -> &Quantity {
        &self.balance
    }

    /// SKU товара.
    #[must_use]
    pub fn sku(&self) -> &Sku {
        &self.sku
    }
}

impl Entity for InventoryItem {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl AggregateRoot for InventoryItem {
    type Event = GoodsReceived;

    fn apply(&mut self, event: &Self::Event) {
        self.balance = Quantity::new(event.new_balance.clone())
            .expect("new_balance in event must be non-negative");
    }

    fn take_events(&mut self) -> Vec<Self::Event> {
        std::mem::take(&mut self.pending_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;

    fn make_item() -> InventoryItem {
        let sku = Sku::new("BOLT-42").unwrap();
        InventoryItem::new(EntityId::new(), sku)
    }

    #[test]
    fn new_item_has_zero_balance() {
        let item = make_item();
        assert!(item.balance().is_zero());
    }

    #[test]
    fn receive_increases_balance() {
        let mut item = make_item();
        let qty = Quantity::new(BigDecimal::from(100)).unwrap();
        item.receive(&qty, "ПРХ-000001".into()).unwrap();

        assert_eq!(item.balance().value(), &BigDecimal::from(100));
    }

    #[test]
    fn receive_cumulative() {
        let mut item = make_item();
        item.receive(
            &Quantity::new(BigDecimal::from(100)).unwrap(),
            "ПРХ-1".into(),
        )
        .unwrap();
        item.receive(
            &Quantity::new(BigDecimal::from(50)).unwrap(),
            "ПРХ-2".into(),
        )
        .unwrap();

        assert_eq!(item.balance().value(), &BigDecimal::from(150));
    }

    #[test]
    fn receive_zero_rejected() {
        let mut item = make_item();
        let result = item.receive(&Quantity::zero(), "ПРХ-1".into());
        assert!(result.is_err());
    }

    #[test]
    fn receive_creates_event() {
        let mut item = make_item();
        let qty = Quantity::new(BigDecimal::from(100)).unwrap();
        let event = item.receive(&qty, "ПРХ-000001".into()).unwrap();

        assert_eq!(event.sku, "BOLT-42");
        assert_eq!(event.quantity, BigDecimal::from(100));
        assert_eq!(event.new_balance, BigDecimal::from(100));
        assert_eq!(event.doc_number, "ПРХ-000001");
    }

    #[test]
    fn take_events_returns_and_clears() {
        let mut item = make_item();
        item.receive(
            &Quantity::new(BigDecimal::from(100)).unwrap(),
            "ПРХ-1".into(),
        )
        .unwrap();
        item.receive(
            &Quantity::new(BigDecimal::from(50)).unwrap(),
            "ПРХ-2".into(),
        )
        .unwrap();

        let events = item.take_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].quantity, BigDecimal::from(100));
        assert_eq!(events[1].quantity, BigDecimal::from(50));

        let events_again = item.take_events();
        assert!(events_again.is_empty());
    }

    #[test]
    fn from_state_restores_balance() {
        let sku = Sku::new("NUT-7").unwrap();
        let balance = Quantity::new(BigDecimal::from(200)).unwrap();
        let item = InventoryItem::from_state(EntityId::new(), sku, balance);

        assert_eq!(item.balance().value(), &BigDecimal::from(200));
        assert_eq!(item.sku().as_str(), "NUT-7");
    }

    #[test]
    fn entity_id_is_uuid_v7() {
        let item = make_item();
        assert_eq!(item.id().as_uuid().get_version_num(), 7);
    }
}
