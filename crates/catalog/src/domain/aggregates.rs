//! Агрегат `Product` — корень консистентности для товара в каталоге.

use kernel::entity::{AggregateRoot, Entity};
use kernel::types::{EntityId, TenantId};

use super::events::ProductCreated;
use super::value_objects::{ProductName, Sku};

/// Товар в каталоге.
///
/// Минимальная сущность: SKU, наименование, категория, единица измерения.
pub struct Product {
    id: EntityId,
    sku: Sku,
    name: ProductName,
    category: String,
    unit: String,
    pending_events: Vec<ProductCreated>,
}

impl Product {
    /// Создать новый товар. Генерирует `ProductCreated` event.
    #[must_use]
    pub fn create(
        id: EntityId,
        tenant_id: TenantId,
        sku: Sku,
        name: ProductName,
        category: String,
        unit: String,
    ) -> Self {
        let event = ProductCreated {
            tenant_id: *tenant_id.as_uuid(),
            product_id: *id.as_uuid(),
            sku: sku.as_str().to_string(),
            name: name.as_str().to_string(),
            category: category.clone(),
            unit: unit.clone(),
        };

        Self {
            id,
            sku,
            name,
            category,
            unit,
            pending_events: vec![event],
        }
    }

    /// SKU товара.
    #[must_use]
    pub fn sku(&self) -> &Sku {
        &self.sku
    }

    /// Наименование товара.
    #[must_use]
    pub fn name(&self) -> &ProductName {
        &self.name
    }

    /// Категория товара.
    #[must_use]
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Единица измерения.
    #[must_use]
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

impl Entity for Product {
    fn id(&self) -> EntityId {
        self.id
    }
}

impl AggregateRoot for Product {
    type Event = ProductCreated;

    fn apply(&mut self, event: &Self::Event) {
        // Product is immutable after creation in MVP.
        // apply is called for event replay; update fields from event.
        self.category.clone_from(&event.category);
        self.unit.clone_from(&event.unit);
    }

    fn take_events(&mut self) -> Vec<Self::Event> {
        std::mem::take(&mut self.pending_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_product() -> Product {
        let sku = Sku::new("BOLT-42").unwrap();
        let name = ProductName::new("Болт M10x50").unwrap();
        Product::create(
            EntityId::new(),
            TenantId::new(),
            sku,
            name,
            "Крепёж".into(),
            "шт".into(),
        )
    }

    #[test]
    fn create_product_sets_fields() {
        let product = make_product();
        assert_eq!(product.sku().as_str(), "BOLT-42");
        assert_eq!(product.name().as_str(), "Болт M10x50");
        assert_eq!(product.category(), "Крепёж");
        assert_eq!(product.unit(), "шт");
    }

    #[test]
    fn create_product_generates_event() {
        let mut product = make_product();
        let events = product.take_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sku, "BOLT-42");
        assert_eq!(events[0].name, "Болт M10x50");
        assert_eq!(events[0].category, "Крепёж");
        assert_eq!(events[0].unit, "шт");
    }

    #[test]
    fn take_events_clears_pending() {
        let mut product = make_product();
        let events = product.take_events();
        assert_eq!(events.len(), 1);

        let events_again = product.take_events();
        assert!(events_again.is_empty());
    }

    #[test]
    fn entity_id_is_uuid_v7() {
        let product = make_product();
        assert_eq!(product.id().as_uuid().get_version_num(), 7);
    }

    #[test]
    fn event_type_is_correct() {
        use kernel::DomainEvent;
        let mut product = make_product();
        let events = product.take_events();
        assert_eq!(events[0].event_type(), "erp.catalog.product_created.v1");
    }
}
