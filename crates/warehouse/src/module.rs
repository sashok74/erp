//! `WarehouseModule` — регистрация Warehouse BC в приложении.

use std::sync::Arc;

use async_trait::async_trait;
use event_bus::{EventBus, EventHandlerAdapter};
use runtime::BoundedContextModule;

use crate::infrastructure::event_handlers::ProductCreatedHandler;

/// Warehouse Bounded Context module.
pub struct WarehouseModule {
    pool: Arc<db::PgPool>,
}

impl WarehouseModule {
    /// Создать модуль с необходимыми зависимостями.
    #[must_use]
    pub fn new(pool: Arc<db::PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BoundedContextModule for WarehouseModule {
    fn name(&self) -> &'static str {
        "warehouse"
    }

    fn migrations_dir(&self) -> &'static str {
        "migrations/warehouse"
    }

    async fn register_handlers(&self, bus: &dyn EventBus) {
        let handler = ProductCreatedHandler::new(self.pool.clone());
        let adapter = Arc::new(EventHandlerAdapter::new(handler));
        bus.subscribe("erp.catalog.product_created.v1", adapter)
            .await;
    }
}
