//! Event handlers для Warehouse BC.
//!
//! Обработка integration events из других Bounded Contexts.
//! Warehouse НЕ зависит от catalog crate — использует свою копию структуры события.

use std::sync::Arc;

use async_trait::async_trait;
use kernel::DomainEvent;
use kernel::types::TenantId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Локальное представление события `ProductCreated` из Catalog BC.
///
/// Decoupled: warehouse НЕ зависит от catalog crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCreatedEvent {
    pub tenant_id: Uuid,
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit: String,
}

impl DomainEvent for ProductCreatedEvent {
    fn event_type(&self) -> &'static str {
        "erp.catalog.product_created.v1"
    }

    fn aggregate_id(&self) -> Uuid {
        self.product_id
    }
}

/// Handler: при создании товара в каталоге — upsert проекции в warehouse.
pub struct ProductCreatedHandler {
    pool: Arc<db::PgPool>,
}

impl ProductCreatedHandler {
    #[must_use]
    pub fn new(pool: Arc<db::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl event_bus::traits::EventHandler for ProductCreatedHandler {
    type Event = ProductCreatedEvent;

    async fn handle(&self, event: &Self::Event) -> Result<(), anyhow::Error> {
        let client = self.pool.get().await?;

        // Set RLS context
        db::rls::set_tenant_context(&**client, TenantId::from_uuid(event.tenant_id)).await?;

        // Upsert product projection
        clorinde_gen::queries::warehouse::projections::upsert_product_projection()
            .bind(
                &**client,
                &event.tenant_id,
                &event.product_id,
                &event.sku.as_str(),
                &event.name.as_str(),
                &event.category.as_str(),
            )
            .await?;

        tracing::info!(
            sku = %event.sku,
            product_id = %event.product_id,
            "product projection upserted"
        );

        Ok(())
    }

    fn handled_event_type(&self) -> &'static str {
        "erp.catalog.product_created.v1"
    }
}
